use crate::node_system::document::{NodeId, PortAddress, PortInstanceId, PortRef};
use crate::node_system::plan::{
    BranchResultBinding, ControlStep, FunctionPlanHandle, LoopCarriedBinding, OperationIndex,
    RegionValueBinding, StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::{
    ManagedNodeRole, NodeProtocol, ParameterKey, PortDirection, PortKind, PortMemberGroupSpec,
};
use crate::node_system::registry::StructuralNodeRole;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FlowNode {
    Node(NodeId),
    Exit,
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
        self.walk_stopping_before(node_id, None)
    }

    fn walk_stopping_before(
        &mut self,
        node_id: NodeId,
        stop: Option<NodeId>,
    ) -> Result<StructuredControlRegion, ControlIssue> {
        if stop == Some(node_id) {
            return Ok(empty_region());
        }
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
                    append_region(&mut steps, self.walk_stopping_before(successor, stop)?);
                }
                Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
            }
            Some(StructuralNodeRole::Sequence) => {
                let mut steps = Vec::new();
                for successor in self.successors(node_id, Some("then")) {
                    append_region(&mut steps, self.walk_stopping_before(successor, stop)?);
                }
                Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
            }
            Some(StructuralNodeRole::Branch) => {
                let condition = self.value_for_key(node_id, "condition")?;
                let continuation = self.branch_continuation(node_id)?;
                let then_region =
                    self.single_region_stopping_before(node_id, "true", continuation.or(stop))?;
                let else_region =
                    self.single_region_stopping_before(node_id, "false", continuation.or(stop))?;
                let results = self.branch_results(node_id)?;
                let branch = StructuredControlRegion::If {
                    condition,
                    then_region: Box::new(then_region),
                    else_region: Box::new(else_region),
                    results: results.into_boxed_slice(),
                };
                match continuation {
                    Some(continuation) if Some(continuation) != stop => {
                        let continuation = self.walk_stopping_before(continuation, stop)?;
                        let mut steps = vec![ControlStep::Region(Box::new(branch))];
                        append_region(&mut steps, continuation);
                        Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
                    }
                    _ => Ok(branch),
                }
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
                self.with_continuation(node_id, loop_region, &["then", "completed", "exit"], stop)
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
                self.with_continuation(node_id, call, &["then", "completed", "exit"], stop)
            }
            Some(StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry) => {
                let mut steps = Vec::new();
                for successor in self.successors(node_id, None) {
                    append_region(&mut steps, self.walk_stopping_before(successor, stop)?);
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
        stop: Option<NodeId>,
    ) -> Result<StructuredControlRegion, ControlIssue> {
        let mut steps = vec![ControlStep::Region(Box::new(region))];
        for key in keys {
            for successor in self.successors(node_id, Some(key)) {
                append_region(&mut steps, self.walk_stopping_before(successor, stop)?);
            }
        }
        Ok(StructuredControlRegion::Sequence(steps.into_boxed_slice()))
    }

    fn single_region(
        &mut self,
        node_id: NodeId,
        key: &str,
    ) -> Result<StructuredControlRegion, ControlIssue> {
        self.single_region_stopping_before(node_id, key, None)
    }

    fn single_region_stopping_before(
        &mut self,
        node_id: NodeId,
        key: &str,
        stop: Option<NodeId>,
    ) -> Result<StructuredControlRegion, ControlIssue> {
        let successors = self.successors(node_id, Some(key));
        match successors.as_slice() {
            [] => Ok(empty_region()),
            [successor] => self.walk_stopping_before(*successor, stop),
            _ => Err(issue(
                node_id,
                "compiler.control.ambiguous_output",
                "structural output has multiple successors",
            )),
        }
    }

    fn branch_continuation(&self, node_id: NodeId) -> Result<Option<NodeId>, ControlIssue> {
        let then_start = self.branch_arm_start(node_id, "true")?;
        let else_start = self.branch_arm_start(node_id, "false")?;
        let graph = self.normal_flow_graph()?;
        let post_dominators = post_dominators(&graph).ok_or_else(|| {
            issue(
                node_id,
                "compiler.control.cycle",
                "normal control flow must reach a structural exit",
            )
        })?;
        let then_post_dominators = &post_dominators[&then_start];
        let else_post_dominators = &post_dominators[&else_start];
        let common = then_post_dominators
            .intersection(else_post_dominators)
            .copied()
            .filter(|candidate| *candidate != FlowNode::Exit)
            .collect::<BTreeSet<_>>();
        let immediate = common
            .iter()
            .copied()
            .filter(|candidate| {
                !common
                    .iter()
                    .copied()
                    .any(|other| other != *candidate && post_dominators[&other].contains(candidate))
            })
            .collect::<Vec<_>>();
        match immediate.as_slice() {
            [FlowNode::Node(continuation)] => Ok(Some(*continuation)),
            [] => {
                let then_reachable = reachable_flow_nodes(&graph, then_start);
                let else_reachable = reachable_flow_nodes(&graph, else_start);
                if then_reachable
                    .intersection(&else_reachable)
                    .any(|candidate| *candidate != FlowNode::Exit)
                {
                    Err(issue(
                        node_id,
                        "compiler.control.unstructured_continuation",
                        "branch arms share reachable nodes that do not post-dominate every arm path",
                    ))
                } else {
                    Ok(None)
                }
            }
            _ => Err(issue(
                node_id,
                "compiler.control.branch.continuation_ambiguous",
                "branch arms have multiple incomparable immediate post-dominators",
            )),
        }
    }

    fn branch_arm_start(&self, node_id: NodeId, key: &str) -> Result<FlowNode, ControlIssue> {
        match self.successors(node_id, Some(key)).as_slice() {
            [] => Ok(FlowNode::Exit),
            [successor] => Ok(FlowNode::Node(*successor)),
            _ => Err(issue(
                node_id,
                "compiler.control.ambiguous_output",
                "branch arm has multiple successors",
            )),
        }
    }

    fn normal_flow_graph(&self) -> Result<BTreeMap<FlowNode, BTreeSet<FlowNode>>, ControlIssue> {
        let mut graph = BTreeMap::new();
        graph.insert(FlowNode::Exit, BTreeSet::new());
        for &node_id in self.nodes.keys() {
            let node = &self.nodes[&node_id];
            let mut successors = match node.role {
                Some(StructuralNodeRole::Branch) => {
                    let mut successors = BTreeSet::new();
                    successors.insert(self.branch_arm_start(node_id, "true")?);
                    successors.insert(self.branch_arm_start(node_id, "false")?);
                    successors
                }
                Some(StructuralNodeRole::Loop) => self
                    .successors_for_keys(node_id, &["then", "completed", "exit"])
                    .into_iter()
                    .map(FlowNode::Node)
                    .collect(),
                Some(StructuralNodeRole::Call) => self
                    .successors_for_keys(node_id, &["then", "completed", "exit"])
                    .into_iter()
                    .map(FlowNode::Node)
                    .collect(),
                Some(StructuralNodeRole::FunctionReturn) => BTreeSet::new(),
                _ => self
                    .successors(node_id, None)
                    .into_iter()
                    .map(FlowNode::Node)
                    .collect(),
            };
            if successors.is_empty() {
                successors.insert(FlowNode::Exit);
            }
            graph.insert(FlowNode::Node(node_id), successors);
        }
        Ok(graph)
    }

    fn successors_for_keys(&self, node_id: NodeId, keys: &[&str]) -> BTreeSet<NodeId> {
        keys.iter()
            .flat_map(|key| self.successors(node_id, Some(key)))
            .collect()
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

    fn resolve_call_value(&self, node_id: NodeId, value: &Value) -> Result<ValueRef, ControlIssue> {
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
        self.grouped_values(
            node_id,
            &[
                ("then_source", PortDirection::Input),
                ("else_source", PortDirection::Input),
                ("result", PortDirection::Output),
            ],
        )?
        .into_iter()
        .map(|values| {
            Ok(BranchResultBinding {
                destination: values["result"],
                then_source: values["then_source"],
                else_source: values["else_source"],
            })
        })
        .collect()
    }

    fn loop_carried(&self, node_id: NodeId) -> Result<Vec<LoopCarriedBinding>, ControlIssue> {
        self.grouped_values(
            node_id,
            &[
                ("initial_source", PortDirection::Input),
                ("body_input", PortDirection::Output),
                ("next_source", PortDirection::Input),
                ("result", PortDirection::Output),
            ],
        )?
        .into_iter()
        .map(|values| {
            Ok(LoopCarriedBinding {
                body_input: values["body_input"],
                initial_source: values["initial_source"],
                next_source: values["next_source"],
                result: values["result"],
            })
        })
        .collect()
    }

    fn grouped_values(
        &self,
        node_id: NodeId,
        expected: &[(&str, PortDirection)],
    ) -> Result<Vec<BTreeMap<Box<str>, ValueRef>>, ControlIssue> {
        let node = &self.nodes[&node_id];
        let expected_keys = expected
            .iter()
            .map(|(key, _)| *key)
            .collect::<BTreeSet<_>>();
        let groups = node
            .protocol
            .interface
            .member_groups
            .iter()
            .filter(|group| {
                group.templates.len() == expected_keys.len()
                    && group
                        .templates
                        .iter()
                        .all(|template| expected_keys.contains(template.as_str()))
            })
            .collect::<Vec<_>>();
        let group = match groups.as_slice() {
            [group] => *group,
            [] => {
                return Err(issue(
                    node_id,
                    "compiler.control.member_group_missing",
                    "structural node is missing its required port member group contract",
                ));
            }
            _ => {
                return Err(issue(
                    node_id,
                    "compiler.control.member_group_ambiguous",
                    "structural node has multiple matching port member group contracts",
                ));
            }
        };
        self.validate_group_directions(node_id, group, expected)?;

        let mut present = BTreeMap::<PortInstanceId, BTreeSet<&str>>::new();
        for address in node.values.keys() {
            let PortRef::Instance {
                template,
                instance_id,
            } = &address.port
            else {
                continue;
            };
            if group.templates.contains(template) {
                present
                    .entry(*instance_id)
                    .or_default()
                    .insert(template.as_str());
            }
        }
        let partial = present
            .iter()
            .filter(|(_, templates)| !templates.is_superset(&expected_keys))
            .collect::<Vec<_>>();
        if !partial.is_empty() {
            let union = partial
                .iter()
                .flat_map(|(_, templates)| templates.iter().copied())
                .collect::<BTreeSet<_>>();
            let (code, detail) = if partial.len() > 1 && union == expected_keys {
                (
                    "compiler.control.member_group_identity_ambiguous",
                    "grouped endpoints are split across incompatible shared instance IDs",
                )
            } else {
                (
                    "compiler.control.member_group_incomplete",
                    "a shared instance ID is missing one or more grouped endpoints",
                )
            };
            return Err(issue(node_id, code, detail));
        }
        let complete_instances = present.keys().copied().collect::<BTreeSet<_>>();
        if complete_instances.len() < group.min as usize
            || group
                .max
                .is_some_and(|max| complete_instances.len() > max as usize)
        {
            return Err(issue(
                node_id,
                "compiler.control.member_group_count_invalid",
                "complete structural member count is outside the protocol bounds",
            ));
        }

        complete_instances
            .into_iter()
            .map(|instance_id| {
                let mut values = BTreeMap::new();
                for (key, _) in expected {
                    let template = group
                        .templates
                        .iter()
                        .find(|template| template.as_str() == *key)
                        .expect("matching group contains every expected template");
                    let address = PortAddress::instance(node_id, template.clone(), instance_id);
                    let value = node.values[&address];
                    values.insert(Box::<str>::from(*key), value);
                }
                Ok(values)
            })
            .collect()
    }

    fn validate_group_directions(
        &self,
        node_id: NodeId,
        group: &PortMemberGroupSpec,
        expected: &[(&str, PortDirection)],
    ) -> Result<(), ControlIssue> {
        let protocol = self.nodes[&node_id].protocol;
        for (key, direction) in expected {
            let valid = group
                .templates
                .iter()
                .any(|template| template.as_str() == *key)
                && protocol.interface.ports.iter().any(|port| {
                    port.key.as_str() == *key
                        && port.kind == PortKind::Data
                        && port.direction == *direction
                });
            if !valid {
                return Err(issue(
                    node_id,
                    "compiler.control.member_group_direction_invalid",
                    &format!("grouped endpoint {key} has the wrong data direction"),
                ));
            }
        }
        Ok(())
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
                        destination: self.resolve_call_value(node_id, &item["destination"])?,
                        source: self.resolve_call_value(node_id, &item["source"])?,
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

fn reachable_flow_nodes(
    graph: &BTreeMap<FlowNode, BTreeSet<FlowNode>>,
    start: FlowNode,
) -> BTreeSet<FlowNode> {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([start]);
    while let Some(node) = pending.pop_front() {
        if !reachable.insert(node) {
            continue;
        }
        pending.extend(graph.get(&node).into_iter().flatten().copied());
    }
    reachable
}

fn post_dominators(
    graph: &BTreeMap<FlowNode, BTreeSet<FlowNode>>,
) -> Option<BTreeMap<FlowNode, BTreeSet<FlowNode>>> {
    let mut reverse = BTreeMap::<FlowNode, BTreeSet<FlowNode>>::new();
    for (&node, successors) in graph {
        reverse.entry(node).or_default();
        for &successor in successors {
            reverse.entry(successor).or_default().insert(node);
        }
    }
    let can_reach_exit = reachable_flow_nodes(&reverse, FlowNode::Exit);
    if can_reach_exit.len() != graph.len() {
        return None;
    }

    let vertices = graph.keys().copied().collect::<BTreeSet<_>>();
    let mut sets = graph
        .keys()
        .copied()
        .map(|node| {
            let initial = if node == FlowNode::Exit {
                BTreeSet::from([FlowNode::Exit])
            } else {
                vertices.clone()
            };
            (node, initial)
        })
        .collect::<BTreeMap<_, _>>();
    let iteration_limit = vertices.len().saturating_mul(vertices.len()).max(1);
    for _ in 0..iteration_limit {
        let mut changed = false;
        let previous = sets.clone();
        for (&node, successors) in graph {
            if node == FlowNode::Exit {
                continue;
            }
            let mut successor_sets = successors.iter().map(|successor| &previous[successor]);
            let mut next = successor_sets.next()?.clone();
            for successor_set in successor_sets {
                next = next.intersection(successor_set).copied().collect();
            }
            next.insert(node);
            if sets[&node] != next {
                sets.insert(node, next);
                changed = true;
            }
        }
        if !changed {
            return Some(sets);
        }
    }
    None
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
