use crate::node_system::document::{NodeId, PortAddress, PortRef};
use crate::node_system::plan::{
    BranchResultBinding, ControlStep, FunctionPlanHandle, LoopCarriedBinding, OperationIndex,
    RegionValueBinding, StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::{
    ManagedNodeRole, NodeProtocol, ParameterKey, PortDirection, PortKind,
};
use crate::node_system::registry::StructuralNodeRole;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

pub(crate) struct ControlNode<'a> {
    pub node_id: NodeId,
    pub role: Option<StructuralNodeRole>,
    pub protocol: &'a NodeProtocol,
    pub parameters: &'a BTreeMap<ParameterKey, Value>,
    pub ports: Box<[PortAddress]>,
    pub values: BTreeMap<PortAddress, ValueRef>,
    pub operation: Option<OperationIndex>,
}

pub(crate) struct ControlEdge {
    pub source: PortAddress,
    pub target: PortAddress,
}

#[derive(Debug)]
pub(crate) struct ControlIssue {
    pub node_id: Option<NodeId>,
    pub code: &'static str,
    pub detail: String,
}

pub(crate) fn validate_structural_contract(
    node_id: NodeId,
    role: StructuralNodeRole,
    protocol: &NodeProtocol,
    parameters: &BTreeMap<ParameterKey, Value>,
) -> Vec<ControlIssue> {
    let mut issues = Vec::new();
    let expected_managed_role = match role {
        StructuralNodeRole::EventBegin => Some(ManagedNodeRole::EventBegin),
        StructuralNodeRole::FunctionEntry => Some(ManagedNodeRole::FunctionEntry),
        StructuralNodeRole::FunctionReturn => Some(ManagedNodeRole::FunctionReturn),
        _ => None,
    };
    if protocol.managed_role != expected_managed_role {
        issues.push(issue(
            node_id,
            "compiler.control.managed_role_mismatch",
            "structural role and protocol managed role do not match",
        ));
    }
    match role {
        StructuralNodeRole::Branch => {
            require_data_port(
                &mut issues,
                node_id,
                protocol,
                "condition",
                PortDirection::Input,
            );
            require_control_port(
                &mut issues,
                node_id,
                protocol,
                "true",
                PortDirection::Output,
            );
            require_control_port(
                &mut issues,
                node_id,
                protocol,
                "false",
                PortDirection::Output,
            );
            validate_binding_parameter(
                &mut issues,
                node_id,
                protocol,
                parameters,
                "results",
                &["destination", "then_source", "else_source"],
                false,
            );
        }
        StructuralNodeRole::Loop => {
            require_data_port(
                &mut issues,
                node_id,
                protocol,
                "condition",
                PortDirection::Input,
            );
            require_control_port(
                &mut issues,
                node_id,
                protocol,
                "body",
                PortDirection::Output,
            );
            match parameter(parameters, "max_iterations").and_then(Value::as_u64) {
                Some(value) if value > 0 => {}
                _ => issues.push(issue(
                    node_id,
                    "compiler.control.loop.max_iterations_required",
                    "loop requires a positive integer max_iterations parameter",
                )),
            }
            validate_binding_parameter(
                &mut issues,
                node_id,
                protocol,
                parameters,
                "carried",
                &["body_input", "initial_source", "next_source", "result"],
                true,
            );
        }
        StructuralNodeRole::Call => {
            if call_target(parameters).is_none() {
                issues.push(issue(
                    node_id,
                    "compiler.control.call.resource_parameter_missing",
                    "call requires a non-empty target/function_plan resource parameter",
                ));
            }
            validate_binding_parameter(
                &mut issues,
                node_id,
                protocol,
                parameters,
                "arguments",
                &["destination", "source"],
                false,
            );
            validate_binding_parameter(
                &mut issues,
                node_id,
                protocol,
                parameters,
                "results",
                &["destination", "source"],
                false,
            );
        }
        StructuralNodeRole::Sequence => {
            require_control_port(
                &mut issues,
                node_id,
                protocol,
                "then",
                PortDirection::Output,
            );
        }
        StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry => {
            if !has_port(protocol, PortKind::Control, PortDirection::Output, None) {
                issues.push(issue(
                    node_id,
                    "compiler.control.entry.output_required",
                    "entry structural node requires a control output",
                ));
            }
        }
        StructuralNodeRole::FunctionReturn => {
            if !has_port(protocol, PortKind::Control, PortDirection::Input, None) {
                issues.push(issue(
                    node_id,
                    "compiler.control.return.input_required",
                    "function return requires a control input",
                ));
            }
        }
    }
    issues
}

pub(crate) fn build_control_region(
    nodes: BTreeMap<NodeId, ControlNode<'_>>,
    edges: Vec<ControlEdge>,
) -> Result<StructuredControlRegion, ControlIssue> {
    let mut builder = RegionBuilder::new(nodes, edges)?;
    builder.build()
}

struct RegionBuilder<'a> {
    nodes: BTreeMap<NodeId, ControlNode<'a>>,
    outgoing: BTreeMap<PortAddress, Vec<NodeId>>,
    incoming: BTreeMap<NodeId, usize>,
    visited: BTreeSet<NodeId>,
    active: BTreeSet<NodeId>,
}

impl<'a> RegionBuilder<'a> {
    fn new(
        nodes: BTreeMap<NodeId, ControlNode<'a>>,
        edges: Vec<ControlEdge>,
    ) -> Result<Self, ControlIssue> {
        let mut outgoing: BTreeMap<PortAddress, Vec<NodeId>> = BTreeMap::new();
        let mut incoming = BTreeMap::new();
        for edge in edges {
            if !nodes.contains_key(&edge.source.node_id)
                || !nodes.contains_key(&edge.target.node_id)
            {
                continue;
            }
            outgoing
                .entry(edge.source)
                .or_default()
                .push(edge.target.node_id);
            *incoming.entry(edge.target.node_id).or_insert(0) += 1;
        }
        for targets in outgoing.values_mut() {
            targets.sort_unstable();
            targets.dedup();
            if targets.len() > 1 {
                return Err(ControlIssue {
                    node_id: None,
                    code: "compiler.control.ambiguous_output",
                    detail: "a control output may enter only one structured region".into(),
                });
            }
        }
        Ok(Self {
            nodes,
            outgoing,
            incoming,
            visited: BTreeSet::new(),
            active: BTreeSet::new(),
        })
    }

    fn build(&mut self) -> Result<StructuredControlRegion, ControlIssue> {
        let entries: Vec<_> = self
            .nodes
            .values()
            .filter(|node| {
                node.role == Some(StructuralNodeRole::EventBegin)
                    || node.role == Some(StructuralNodeRole::FunctionEntry)
            })
            .map(|node| node.node_id)
            .collect();
        let roots = if entries.is_empty() {
            self.nodes
                .keys()
                .filter(|id| self.incoming.get(id).copied().unwrap_or(0) == 0)
                .copied()
                .collect::<Vec<_>>()
        } else {
            entries
        };
        if !self.nodes.is_empty() && roots.is_empty() {
            return Err(ControlIssue {
                node_id: None,
                code: "compiler.control.no_entry",
                detail: "control graph has no structural entry".into(),
            });
        }
        let mut steps = Vec::new();
        for root in roots {
            append_region(&mut steps, self.walk(root)?);
        }
        if self.visited.len() != self.nodes.len() {
            let node_id = self
                .nodes
                .keys()
                .find(|id| !self.visited.contains(id))
                .copied();
            return Err(ControlIssue {
                node_id,
                code: "compiler.control.unreachable",
                detail: "node is not part of a recognized structured control region".into(),
            });
        }
        Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
    }

    fn walk(&mut self, node_id: NodeId) -> Result<StructuredControlRegion, ControlIssue> {
        if self.active.contains(&node_id) {
            return Err(issue(
                node_id,
                "compiler.control.cycle",
                "control cycles must be represented by an explicit Loop node",
            ));
        }
        if self.visited.contains(&node_id) {
            return Err(issue(
                node_id,
                "compiler.control.shared_region",
                "a node cannot belong to more than one structured region",
            ));
        }
        self.active.insert(node_id);
        self.visited.insert(node_id);
        let (role, operation) = {
            let node = &self.nodes[&node_id];
            (node.role, node.operation)
        };
        let result = match role {
            None => {
                let operation = operation.ok_or_else(|| {
                    issue(
                        node_id,
                        "compiler.control.leaf_without_operation",
                        "leaf node has no operation",
                    )
                })?;
                let mut steps = vec![ControlStep::Operation(operation)];
                for successor in self.successors(node_id, None) {
                    append_region(&mut steps, self.walk(successor)?);
                }
                Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
            }
            Some(StructuralNodeRole::Sequence) => {
                let mut steps = Vec::new();
                for successor in self.successors(node_id, Some("then")) {
                    append_region(&mut steps, self.walk(successor)?);
                }
                Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
            }
            Some(StructuralNodeRole::Branch) => {
                let condition = self.value_for_key(node_id, "condition")?;
                let then_region = self.single_region(node_id, "true")?;
                let else_region = self.single_region(node_id, "false")?;
                let results = self.branch_results(node_id)?;
                Ok(StructuredControlRegion::If {
                    condition,
                    then_region: Box::new(then_region),
                    else_region: Box::new(else_region),
                    results: results.into_boxed_slice(),
                })
            }
            Some(StructuralNodeRole::Loop) => {
                let condition = self.value_for_key(node_id, "condition")?;
                let body = self.single_region(node_id, "body")?;
                let carried = self.loop_carried(node_id)?;
                let max_iterations = parameter(self.nodes[&node_id].parameters, "max_iterations")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| {
                        issue(
                            node_id,
                            "compiler.control.loop.max_iterations_required",
                            "loop max_iterations is invalid",
                        )
                    })?;
                let loop_region = StructuredControlRegion::Loop {
                    body: Box::new(body),
                    carried: carried.into_boxed_slice(),
                    continue_condition: condition,
                    max_iterations,
                };
                self.with_continuation(node_id, loop_region, &["then", "completed", "exit"])
            }
            Some(StructuralNodeRole::Call) => {
                let target = FunctionPlanHandle::new(
                    call_target(self.nodes[&node_id].parameters).ok_or_else(|| {
                        issue(
                            node_id,
                            "compiler.control.call.resource_parameter_missing",
                            "call target is missing",
                        )
                    })?,
                )
                .map_err(|error| {
                    issue(
                        node_id,
                        "compiler.control.call.target_invalid",
                        &error.to_string(),
                    )
                })?;
                let arguments = self.call_bindings(node_id, "arguments", PortDirection::Input)?;
                let results = self.call_bindings(node_id, "results", PortDirection::Output)?;
                let call = StructuredControlRegion::Call {
                    target,
                    arguments: arguments.into_boxed_slice(),
                    results: results.into_boxed_slice(),
                };
                self.with_continuation(node_id, call, &["then", "completed", "exit"])
            }
            Some(StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry) => {
                let mut steps = Vec::new();
                for successor in self.successors(node_id, None) {
                    append_region(&mut steps, self.walk(successor)?);
                }
                Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
            }
            Some(StructuralNodeRole::FunctionReturn) => {
                if !self.successors(node_id, None).is_empty() {
                    Err(issue(
                        node_id,
                        "compiler.control.return_has_successor",
                        "function return must terminate its region",
                    ))
                } else {
                    Ok(empty_region())
                }
            }
        };
        self.active.remove(&node_id);
        result
    }

    fn with_continuation(
        &mut self,
        node_id: NodeId,
        region: StructuredControlRegion,
        keys: &[&str],
    ) -> Result<StructuredControlRegion, ControlIssue> {
        let mut steps = vec![ControlStep::Region(Box::new(region))];
        for key in keys {
            for successor in self.successors(node_id, Some(key)) {
                append_region(&mut steps, self.walk(successor)?);
            }
        }
        Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
    }

    fn single_region(
        &mut self,
        node_id: NodeId,
        key: &str,
    ) -> Result<StructuredControlRegion, ControlIssue> {
        let successors = self.successors(node_id, Some(key));
        match successors.as_slice() {
            [] => Ok(empty_region()),
            [successor] => self.walk(*successor),
            _ => Err(issue(
                node_id,
                "compiler.control.ambiguous_output",
                "structural output has multiple successors",
            )),
        }
    }

    fn successors(&self, node_id: NodeId, key: Option<&str>) -> Vec<NodeId> {
        let node = &self.nodes[&node_id];
        let mut ports: Vec<_> = node
            .ports
            .iter()
            .filter(|address| {
                let Some(spec) = port_spec(node.protocol, address) else {
                    return false;
                };
                spec.kind == PortKind::Control
                    && spec.direction == PortDirection::Output
                    && key.is_none_or(|expected| spec.key.as_str() == expected)
            })
            .collect();
        ports.sort();
        ports
            .into_iter()
            .filter_map(|port| {
                self.outgoing
                    .get(port)
                    .and_then(|targets| targets.first())
                    .copied()
            })
            .collect()
    }

    fn value_for_key(&self, node_id: NodeId, key: &str) -> Result<ValueRef, ControlIssue> {
        let node = &self.nodes[&node_id];
        values_for_key(node, key).into_iter().next().ok_or_else(|| {
            issue(
                node_id,
                "compiler.control.value_missing",
                &format!("missing value for port {key}"),
            )
        })
    }

    fn resolve_value(&self, node_id: NodeId, value: &Value) -> Result<ValueRef, ControlIssue> {
        if let Some(index) = value.as_u64().and_then(|value| u32::try_from(value).ok()) {
            return Ok(ValueRef::new(index));
        }
        let key = value
            .as_str()
            .or_else(|| value.get("port").and_then(Value::as_str))
            .ok_or_else(|| {
                issue(
                    node_id,
                    "compiler.control.binding_invalid",
                    "binding value must be a port key or value index",
                )
            })?;
        self.value_for_key(node_id, key)
    }

    fn branch_results(&self, node_id: NodeId) -> Result<Vec<BranchResultBinding>, ControlIssue> {
        let Some(items) = parameter(self.nodes[&node_id].parameters, "results") else {
            return Ok(Vec::new());
        };
        items
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .map(|item| {
                Ok(BranchResultBinding {
                    destination: self.resolve_value(node_id, &item["destination"])?,
                    then_source: self.resolve_value(node_id, &item["then_source"])?,
                    else_source: self.resolve_value(node_id, &item["else_source"])?,
                })
            })
            .collect()
    }

    fn loop_carried(&self, node_id: NodeId) -> Result<Vec<LoopCarriedBinding>, ControlIssue> {
        let items = parameter(self.nodes[&node_id].parameters, "carried")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                issue(
                    node_id,
                    "compiler.control.loop.carried_required",
                    "loop carried bindings are missing",
                )
            })?;
        items
            .iter()
            .map(|item| {
                Ok(LoopCarriedBinding {
                    body_input: self.resolve_value(node_id, &item["body_input"])?,
                    initial_source: self.resolve_value(node_id, &item["initial_source"])?,
                    next_source: self.resolve_value(node_id, &item["next_source"])?,
                    result: self.resolve_value(node_id, &item["result"])?,
                })
            })
            .collect()
    }

    fn call_bindings(
        &self,
        node_id: NodeId,
        name: &str,
        inferred_direction: PortDirection,
    ) -> Result<Vec<RegionValueBinding>, ControlIssue> {
        let node = &self.nodes[&node_id];
        if let Some(items) = parameter(node.parameters, name).and_then(Value::as_array) {
            return items
                .iter()
                .map(|item| {
                    Ok(RegionValueBinding {
                        destination: self.resolve_value(node_id, &item["destination"])?,
                        source: self.resolve_value(node_id, &item["source"])?,
                    })
                })
                .collect();
        }
        Ok(node
            .values
            .iter()
            .filter_map(|(address, value)| {
                let spec = port_spec(node.protocol, address)?;
                (spec.kind == PortKind::Data && spec.direction == inferred_direction).then_some(
                    RegionValueBinding {
                        destination: *value,
                        source: *value,
                    },
                )
            })
            .collect())
    }
}

fn validate_binding_parameter(
    issues: &mut Vec<ControlIssue>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    parameters: &BTreeMap<ParameterKey, Value>,
    name: &str,
    fields: &[&str],
    required: bool,
) {
    let Some(value) = parameter(parameters, name) else {
        if required {
            issues.push(issue(
                node_id,
                "compiler.control.binding_required",
                &format!("{name} bindings are required"),
            ));
        }
        return;
    };
    let Some(items) = value.as_array() else {
        issues.push(issue(
            node_id,
            "compiler.control.binding_invalid",
            &format!("{name} must be an array"),
        ));
        return;
    };
    for item in items {
        let Some(binding) = item.as_object() else {
            issues.push(issue(
                node_id,
                "compiler.control.binding_invalid",
                &format!("{name} binding must be an object"),
            ));
            continue;
        };
        for field in fields {
            let Some(reference) = binding.get(*field) else {
                issues.push(issue(
                    node_id,
                    "compiler.control.binding_invalid",
                    &format!("{name}.{field} is required"),
                ));
                continue;
            };
            if let Some(key) = reference
                .as_str()
                .or_else(|| reference.get("port").and_then(Value::as_str))
            {
                if !has_port(protocol, PortKind::Data, PortDirection::Input, Some(key))
                    && !has_port(protocol, PortKind::Data, PortDirection::Output, Some(key))
                {
                    issues.push(issue(
                        node_id,
                        "compiler.control.binding_port_missing",
                        &format!("unknown data port {key}"),
                    ));
                }
            } else if reference
                .as_u64()
                .and_then(|value| u32::try_from(value).ok())
                .is_none()
            {
                issues.push(issue(
                    node_id,
                    "compiler.control.binding_invalid",
                    &format!("{name}.{field} must reference a data port or value index"),
                ));
            }
        }
    }
}

fn require_data_port(
    issues: &mut Vec<ControlIssue>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    key: &str,
    direction: PortDirection,
) {
    if !has_port(protocol, PortKind::Data, direction, Some(key)) {
        issues.push(issue(
            node_id,
            "compiler.control.data_port_required",
            &format!("missing {direction:?} data port {key}"),
        ));
    }
}

fn require_control_port(
    issues: &mut Vec<ControlIssue>,
    node_id: NodeId,
    protocol: &NodeProtocol,
    key: &str,
    direction: PortDirection,
) {
    if !has_port(protocol, PortKind::Control, direction, Some(key)) {
        issues.push(issue(
            node_id,
            "compiler.control.control_port_required",
            &format!("missing {direction:?} control port {key}"),
        ));
    }
}

fn has_port(
    protocol: &NodeProtocol,
    kind: PortKind,
    direction: PortDirection,
    key: Option<&str>,
) -> bool {
    protocol.interface.ports.iter().any(|port| {
        port.kind == kind
            && port.direction == direction
            && key.is_none_or(|expected| port.key.as_str() == expected)
    })
}

fn values_for_key(node: &ControlNode<'_>, key: &str) -> Vec<ValueRef> {
    node.values
        .iter()
        .filter_map(|(address, value)| {
            port_spec(node.protocol, address)
                .is_some_and(|spec| spec.kind == PortKind::Data && spec.key.as_str() == key)
                .then_some(*value)
        })
        .collect()
}

fn port_spec<'a>(
    protocol: &'a NodeProtocol,
    address: &PortAddress,
) -> Option<&'a crate::node_system::protocol::PortSpec> {
    let key = match &address.port {
        PortRef::Declared { key } => key,
        PortRef::Instance { template, .. } => template,
    };
    protocol
        .interface
        .ports
        .iter()
        .find(|spec| &spec.key == key)
}

fn parameter<'a>(parameters: &'a BTreeMap<ParameterKey, Value>, name: &str) -> Option<&'a Value> {
    parameters
        .iter()
        .find_map(|(key, value)| (key.as_str() == name).then_some(value))
}

fn call_target(parameters: &BTreeMap<ParameterKey, Value>) -> Option<Box<str>> {
    ["target", "function_plan", "function"]
        .into_iter()
        .find_map(|name| parameter(parameters, name).and_then(Value::as_str))
        .filter(|value| !value.trim().is_empty() && value.trim() == *value)
        .map(Into::into)
}

fn append_region(steps: &mut Vec<ControlStep>, region: StructuredControlRegion) {
    match region {
        StructuredControlRegion::Sequence(children) => steps.extend(children.into_vec()),
        region => steps.push(ControlStep::Region(Box::new(region))),
    }
}

fn empty_region() -> StructuredControlRegion {
    StructuredControlRegion::Sequence(Box::new([]))
}

fn issue(node_id: NodeId, code: &'static str, detail: &str) -> ControlIssue {
    ControlIssue {
        node_id: Some(node_id),
        code,
        detail: detail.into(),
    }
}
