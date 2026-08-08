use super::{CompilerDiagnostic, ValidatedNodeConfig, managed_node_role_name};
use crate::node_system::document::{
    DynamicMemberLocator, GraphResourcePath, NodeId, PortAddress, PortInstanceId, PortRef,
};
use crate::node_system::plan::{
    BranchResultBinding, CallArgumentBinding, CallResultBinding, ControlStep, FunctionPlanAbi,
    FunctionPlanHandle, LoopCarriedBinding, OperationIndex, StructuredControlRegion, ValueRef,
};
use crate::node_system::protocol::{
    EvaluationPolicy, ManagedNodeRole, NodeProtocol, ParameterKey, PortDirection, PortKind,
    PortMemberGroupSpec,
};
use crate::node_system::registry::StructuralNodeRole;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub(crate) struct ControlNode<'a> {
    pub node_id: NodeId,
    pub role: Option<StructuralNodeRole>,
    pub protocol: &'a NodeProtocol,
    pub parameters: &'a ValidatedNodeConfig,
    pub ports: Box<[PortAddress]>,
    pub values: BTreeMap<PortAddress, ValueRef>,
    pub dynamic_members: BTreeMap<PortAddress, DynamicMemberLocator>,
    pub operation: Option<OperationIndex>,
}

pub(crate) struct ControlEdge {
    pub source: PortAddress,
    pub target: PortAddress,
}

#[derive(Debug)]
pub(crate) struct ControlIssue {
    pub node_id: Option<NodeId>,
    pub diagnostic: CompilerDiagnostic,
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
            CompilerDiagnostic::ControlManagedRoleMismatch {
                expected_role: managed_node_role_name(expected_managed_role).into(),
                actual_role: managed_node_role_name(protocol.managed_role).into(),
            },
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
                    CompilerDiagnostic::ControlLoopMaxIterationsRequired {
                        parameter_key: "max_iterations".into(),
                    },
                )),
            }
        }
        StructuralNodeRole::Call => {
            if call_target(parameters).is_none() {
                issues.push(issue(
                    node_id,
                    CompilerDiagnostic::ControlCallResourceParameterMissing {
                        parameter_key: "target".into(),
                    },
                ));
            }
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
                    CompilerDiagnostic::ControlEntryOutputRequired {},
                ));
            }
        }
        StructuralNodeRole::FunctionReturn => {
            if !has_port(protocol, PortKind::Control, PortDirection::Input, None) {
                issues.push(issue(
                    node_id,
                    CompilerDiagnostic::ControlReturnInputRequired {},
                ));
            }
        }
    }
    issues
}

pub(crate) fn build_control_region(
    nodes: BTreeMap<NodeId, ControlNode<'_>>,
    edges: Vec<ControlEdge>,
    function_abis: &BTreeMap<GraphResourcePath, FunctionPlanAbi>,
) -> Result<StructuredControlRegion, ControlIssue> {
    let mut builder = RegionBuilder::new(nodes, edges, function_abis)?;
    builder.build()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum FlowNode {
    Node(NodeId),
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommonPostDominator {
    None,
    Node(NodeId),
    Ambiguous,
}

struct RegionBuilder<'a> {
    nodes: BTreeMap<NodeId, ControlNode<'a>>,
    function_abis: &'a BTreeMap<GraphResourcePath, FunctionPlanAbi>,
    outgoing: BTreeMap<PortAddress, Vec<NodeId>>,
    incoming: BTreeMap<NodeId, usize>,
    visited: BTreeSet<NodeId>,
    active: BTreeSet<NodeId>,
}

impl<'a> RegionBuilder<'a> {
    fn new(
        nodes: BTreeMap<NodeId, ControlNode<'a>>,
        edges: Vec<ControlEdge>,
        function_abis: &'a BTreeMap<GraphResourcePath, FunctionPlanAbi>,
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
        for (port, targets) in &mut outgoing {
            targets.sort_unstable();
            targets.dedup();
            if targets.len() > 1 {
                return Err(ControlIssue {
                    node_id: None,
                    diagnostic: CompilerDiagnostic::ControlAmbiguousOutput {
                        port: port.to_string().into(),
                    },
                });
            }
        }
        Ok(Self {
            nodes,
            function_abis,
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
            self.nodes
                .values()
                .filter(|node| {
                    node.operation.is_some()
                        && node
                            .protocol
                            .interface
                            .ports
                            .iter()
                            .all(|port| port.kind != PortKind::Control)
                })
                .map(|node| node.node_id)
                .chain(entries)
                .collect()
        };
        if !self.nodes.is_empty() && roots.is_empty() {
            return Err(ControlIssue {
                node_id: None,
                diagnostic: CompilerDiagnostic::ControlNoEntry {},
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
                diagnostic: CompilerDiagnostic::ControlUnreachable {},
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
            return Err(issue(node_id, CompilerDiagnostic::ControlCycle {}));
        }
        if self.visited.contains(&node_id) {
            return Err(issue(node_id, CompilerDiagnostic::ControlSharedRegion {}));
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
                    issue(node_id, CompilerDiagnostic::ControlLeafWithoutOperation {})
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
                let continuation = self.branch_continuation(node_id, stop)?;
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
                let max_iterations_key =
                    ParameterKey::new("max_iterations").expect("built-in parameter key is valid");
                let max_iterations = self.nodes[&node_id]
                    .parameters
                    .int64(&max_iterations_key)
                    .and_then(|value| u64::try_from(value).ok())
                    .ok_or_else(|| {
                        issue(
                            node_id,
                            CompilerDiagnostic::LoweringInternalInvariant {
                                node_type: self.nodes[&node_id].protocol.type_id.to_string().into(),
                            },
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
                let target_path = GraphResourcePath(
                    prepared_call_target(self.nodes[&node_id].parameters)
                        .ok_or_else(|| {
                            issue(
                                node_id,
                                CompilerDiagnostic::LoweringInternalInvariant {
                                    node_type: self.nodes[&node_id]
                                        .protocol
                                        .type_id
                                        .to_string()
                                        .into(),
                                },
                            )
                        })?
                        .into(),
                );
                let target = FunctionPlanHandle::new(target_path.0.clone()).map_err(|_| {
                    issue(
                        node_id,
                        CompilerDiagnostic::ControlCallTargetInvalid {
                            function_path: target_path.0.clone(),
                        },
                    )
                })?;
                let arguments = self.call_argument_bindings(node_id, &target_path)?;
                let results = self.call_result_bindings(node_id, &target_path)?;
                let mandatory = self.nodes[&node_id].protocol.execution.evaluation
                    == EvaluationPolicy::EagerWhenRegionEntered
                    || self.incoming.get(&node_id).copied().unwrap_or(0) > 0
                    || self
                        .outgoing
                        .keys()
                        .any(|address| address.node_id == node_id);
                let call = StructuredControlRegion::Call {
                    target,
                    arguments: arguments.into_boxed_slice(),
                    results: results.into_boxed_slice(),
                    mandatory,
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
                        CompilerDiagnostic::ControlReturnHasSuccessor {},
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
                CompilerDiagnostic::ControlAmbiguousOutput { port: key.into() },
            )),
        }
    }

    fn branch_continuation(
        &self,
        node_id: NodeId,
        stop: Option<NodeId>,
    ) -> Result<Option<NodeId>, ControlIssue> {
        let then_start = flow_node_before_stop(self.branch_arm_start(node_id, "true")?, stop);
        let else_start = flow_node_before_stop(self.branch_arm_start(node_id, "false")?, stop);
        let graph = self.normal_flow_graph(&[then_start, else_start], stop)?;
        let post_dominators = post_dominators(&graph)
            .ok_or_else(|| issue(node_id, CompilerDiagnostic::ControlCycle {}))?;
        if let Some(continuation) =
            resolve_common_post_dominator(node_id, &post_dominators, then_start, else_start)?
        {
            return Ok(Some(continuation));
        }
        let then_reachable = reachable_flow_nodes(&graph, then_start);
        let else_reachable = reachable_flow_nodes(&graph, else_start);
        if then_reachable
            .intersection(&else_reachable)
            .any(|candidate| *candidate != FlowNode::Exit)
        {
            Err(issue(
                node_id,
                CompilerDiagnostic::ControlUnstructuredContinuation {},
            ))
        } else {
            Ok(None)
        }
    }

    fn branch_arm_start(&self, node_id: NodeId, key: &str) -> Result<FlowNode, ControlIssue> {
        match self.successors(node_id, Some(key)).as_slice() {
            [] => Ok(FlowNode::Exit),
            [successor] => Ok(FlowNode::Node(*successor)),
            _ => Err(issue(
                node_id,
                CompilerDiagnostic::ControlAmbiguousOutput { port: key.into() },
            )),
        }
    }

    fn normal_flow_graph(
        &self,
        starts: &[FlowNode],
        stop: Option<NodeId>,
    ) -> Result<BTreeMap<FlowNode, BTreeSet<FlowNode>>, ControlIssue> {
        let mut graph = BTreeMap::from([(FlowNode::Exit, BTreeSet::new())]);
        let mut pending = VecDeque::from(starts.to_vec());
        while let Some(node) = pending.pop_front() {
            let FlowNode::Node(node_id) = node else {
                continue;
            };
            if graph.contains_key(&node) {
                continue;
            }
            let successors = self.normal_flow_successors(node_id, stop)?;
            pending.extend(successors.iter().copied());
            graph.insert(node, successors);
        }
        Ok(graph)
    }

    fn normal_flow_successors(
        &self,
        node_id: NodeId,
        stop: Option<NodeId>,
    ) -> Result<BTreeSet<FlowNode>, ControlIssue> {
        let node = &self.nodes[&node_id];
        if node.role == Some(StructuralNodeRole::Branch) {
            return Ok(BTreeSet::from([
                flow_node_before_stop(self.branch_arm_start(node_id, "true")?, stop),
                flow_node_before_stop(self.branch_arm_start(node_id, "false")?, stop),
            ]));
        }
        // Keep these role-specific keys identical to walk_stopping_before: Branch uses
        // true/false alternatives above; Sequence uses then; Loop/Call use their ordered
        // continuation keys; leaf and entry roles use every output; Return terminates.
        let ordered = match node.role {
            Some(StructuralNodeRole::Sequence) => self.successors(node_id, Some("then")),
            Some(StructuralNodeRole::Loop | StructuralNodeRole::Call) => {
                self.ordered_successors_for_keys(node_id, &["then", "completed", "exit"])
            }
            Some(StructuralNodeRole::FunctionReturn) => Vec::new(),
            None | Some(StructuralNodeRole::EventBegin | StructuralNodeRole::FunctionEntry) => {
                self.successors(node_id, None)
            }
            Some(StructuralNodeRole::Branch) => unreachable!("Branch is handled above"),
        };
        Ok(BTreeSet::from([ordered
            .last()
            .copied()
            .map(FlowNode::Node)
            .map(|successor| flow_node_before_stop(successor, stop))
            .unwrap_or(FlowNode::Exit)]))
    }

    fn ordered_successors_for_keys(&self, node_id: NodeId, keys: &[&str]) -> Vec<NodeId> {
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
                CompilerDiagnostic::ControlValueMissing { port: key.into() },
            )
        })
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
                production: None,
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
                production: None,
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
                    CompilerDiagnostic::ControlMemberGroupMissing {
                        field_name: expected_keys
                            .iter()
                            .copied()
                            .collect::<Vec<_>>()
                            .join(",")
                            .into(),
                    },
                ));
            }
            _ => {
                return Err(issue(
                    node_id,
                    CompilerDiagnostic::ControlMemberGroupAmbiguous {
                        field_name: expected_keys
                            .iter()
                            .copied()
                            .collect::<Vec<_>>()
                            .join(",")
                            .into(),
                    },
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
            let field_name = union.iter().copied().collect::<Vec<_>>().join(",").into();
            let diagnostic = if partial.len() > 1 && union == expected_keys {
                CompilerDiagnostic::ControlMemberGroupIdentityAmbiguous { field_name }
            } else {
                CompilerDiagnostic::ControlMemberGroupIncomplete { field_name }
            };
            return Err(issue(node_id, diagnostic));
        }
        let complete_instances = present.keys().copied().collect::<BTreeSet<_>>();
        if complete_instances.len() < group.min as usize
            || group
                .max
                .is_some_and(|max| complete_instances.len() > max as usize)
        {
            return Err(issue(
                node_id,
                CompilerDiagnostic::ControlMemberGroupCountInvalid {
                    field_name: expected_keys
                        .iter()
                        .copied()
                        .collect::<Vec<_>>()
                        .join(",")
                        .into(),
                },
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
                    CompilerDiagnostic::ControlMemberGroupDirectionInvalid {
                        field_name: (*key).into(),
                    },
                ));
            }
        }
        Ok(())
    }

    fn call_argument_bindings(
        &self,
        node_id: NodeId,
        target: &GraphResourcePath,
    ) -> Result<Vec<CallArgumentBinding>, ControlIssue> {
        let abi = self.call_abi(node_id, target)?;
        let members = self.call_members(node_id, target, PortDirection::Input)?;
        self.validate_call_member_bijection(node_id, &members, &abi.parameters, "argument")?;
        members
            .into_iter()
            .map(|(parameter, caller_source)| {
                let callee_destination =
                    abi.parameters.get(&parameter).copied().ok_or_else(|| {
                        issue(
                            node_id,
                            CompilerDiagnostic::ControlCallAbiMemberMissing {
                                field_name: parameter.0.clone(),
                            },
                        )
                    })?;
                Ok(CallArgumentBinding {
                    caller_source,
                    callee_destination,
                })
            })
            .collect()
    }

    fn call_result_bindings(
        &self,
        node_id: NodeId,
        target: &GraphResourcePath,
    ) -> Result<Vec<CallResultBinding>, ControlIssue> {
        let abi = self.call_abi(node_id, target)?;
        let members = self.call_members(node_id, target, PortDirection::Output)?;
        self.validate_call_member_bijection(node_id, &members, &abi.results, "result")?;
        members
            .into_iter()
            .map(|(parameter, caller_destination)| {
                let callee_source = abi.results.get(&parameter).copied().ok_or_else(|| {
                    issue(
                        node_id,
                        CompilerDiagnostic::ControlCallAbiMemberMissing {
                            field_name: parameter.0.clone(),
                        },
                    )
                })?;
                let production =
                    abi.result_productions
                        .get(&parameter)
                        .copied()
                        .ok_or_else(|| {
                            issue(
                                node_id,
                                CompilerDiagnostic::ControlCallAbiMemberMissing {
                                    field_name: parameter.0.clone(),
                                },
                            )
                        })?;
                Ok(CallResultBinding {
                    callee_source,
                    caller_destination,
                    production: Some(production),
                })
            })
            .collect()
    }

    fn validate_call_member_bijection(
        &self,
        node_id: NodeId,
        members: &[(crate::node_system::document::FunctionParameterId, ValueRef)],
        expected: &BTreeMap<crate::node_system::document::FunctionParameterId, ValueRef>,
        role: &str,
    ) -> Result<(), ControlIssue> {
        let actual = members
            .iter()
            .map(|(parameter, _)| parameter)
            .collect::<BTreeSet<_>>();
        let expected = expected.keys().collect::<BTreeSet<_>>();
        if let Some(missing) = expected.difference(&actual).next() {
            return Err(issue(
                node_id,
                CompilerDiagnostic::ControlCallMemberMissing {
                    member_role: role.into(),
                    member_id: missing.0.clone(),
                },
            ));
        }
        if let Some(unexpected) = actual.difference(&expected).next() {
            return Err(issue(
                node_id,
                CompilerDiagnostic::ControlCallMemberUnexpected {
                    member_role: role.into(),
                    member_id: unexpected.0.clone(),
                },
            ));
        }
        Ok(())
    }

    fn call_abi(
        &self,
        node_id: NodeId,
        target: &GraphResourcePath,
    ) -> Result<&FunctionPlanAbi, ControlIssue> {
        self.function_abis.get(target).ok_or_else(|| {
            issue(
                node_id,
                CompilerDiagnostic::ControlCallAbiMissing {
                    function_path: target.0.clone(),
                },
            )
        })
    }

    fn call_members(
        &self,
        node_id: NodeId,
        target: &GraphResourcePath,
        direction: PortDirection,
    ) -> Result<Vec<(crate::node_system::document::FunctionParameterId, ValueRef)>, ControlIssue>
    {
        let node = &self.nodes[&node_id];
        let mut members = BTreeMap::new();
        for (address, locator) in &node.dynamic_members {
            let Some(spec) = port_spec(node.protocol, address) else {
                continue;
            };
            if spec.kind != PortKind::Data || spec.direction != direction {
                continue;
            }
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = locator
            else {
                return Err(issue(
                    node_id,
                    CompilerDiagnostic::ControlCallLocatorInvalid {
                        port: address.to_string().into(),
                    },
                ));
            };
            if function != target {
                return Err(issue(
                    node_id,
                    CompilerDiagnostic::ControlCallLocatorTargetMismatch {
                        function_path: function.0.clone(),
                    },
                ));
            }
            let value = node.values.get(address).copied().ok_or_else(|| {
                issue(
                    node_id,
                    CompilerDiagnostic::ControlCallValueMissing {
                        port: address.to_string().into(),
                    },
                )
            })?;
            if members.insert(parameter.clone(), value).is_some() {
                return Err(issue(
                    node_id,
                    CompilerDiagnostic::ControlCallLocatorDuplicate {
                        function_path: function.0.clone(),
                        parameter_id: parameter.0.clone(),
                        port: address.to_string().into(),
                    },
                ));
            }
        }
        Ok(members.into_iter().collect())
    }
}

fn flow_node_before_stop(node: FlowNode, stop: Option<NodeId>) -> FlowNode {
    match node {
        FlowNode::Node(node_id) if Some(node_id) == stop => FlowNode::Exit,
        node => node,
    }
}

fn resolve_common_post_dominator(
    branch_id: NodeId,
    post_dominators: &BTreeMap<FlowNode, BTreeSet<FlowNode>>,
    left: FlowNode,
    right: FlowNode,
) -> Result<Option<NodeId>, ControlIssue> {
    match immediate_common_post_dominator(post_dominators, left, right) {
        CommonPostDominator::None => Ok(None),
        CommonPostDominator::Node(node_id) => Ok(Some(node_id)),
        CommonPostDominator::Ambiguous => Err(issue(
            branch_id,
            CompilerDiagnostic::ControlBranchContinuationAmbiguous {},
        )),
    }
}

fn immediate_common_post_dominator(
    post_dominators: &BTreeMap<FlowNode, BTreeSet<FlowNode>>,
    left: FlowNode,
    right: FlowNode,
) -> CommonPostDominator {
    let common = post_dominators[&left]
        .intersection(&post_dominators[&right])
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
        [] => CommonPostDominator::None,
        [FlowNode::Node(node_id)] => CommonPostDominator::Node(*node_id),
        _ => CommonPostDominator::Ambiguous,
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
            CompilerDiagnostic::ControlDataPortRequired {
                port_key: key.into(),
                expected_direction: direction_name(direction).into(),
            },
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
            CompilerDiagnostic::ControlControlPortRequired {
                port_key: key.into(),
                expected_direction: direction_name(direction).into(),
            },
        ));
    }
}

fn direction_name(direction: PortDirection) -> &'static str {
    match direction {
        PortDirection::Input => "input",
        PortDirection::Output => "output",
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
        .map(Into::into)
}

fn prepared_call_target(parameters: &ValidatedNodeConfig) -> Option<Box<str>> {
    ["target", "function_plan", "function"]
        .into_iter()
        .find_map(|name| {
            let key = ParameterKey::new(name).ok()?;
            parameters.resource(&key).map(|resource| resource.as_str())
        })
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

fn issue(node_id: NodeId, diagnostic: CompilerDiagnostic) -> ControlIssue {
    ControlIssue {
        node_id: Some(node_id),
        diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node_system::catalog::build_builtin_node_system;
    use crate::node_system::protocol::{
        I18nKey, NodeInterfaceProtocol, NodeTypeId, ParameterEditorSpec, ParameterSpec, PortKey,
        TypeExpr, TypeId,
    };
    use crate::node_system::testing::TestProtocolBuilder;
    use uuid::Uuid;

    #[test]
    fn prepared_call_target_rejects_string_fallback() {
        let protocol = TestProtocolBuilder::new("yssbi.test.call_config", "test")
            .style("test")
            .parameters(vec![ParameterSpec {
                key: ParameterKey::new("target").unwrap(),
                title_key: I18nKey::new("nodes.test.call_config.target").unwrap(),
                description_key: None,
                value_type: TypeExpr::Concrete(TypeId::new("core.string").unwrap()),
                default_value: None,
                constraints: Vec::new(),
                editor: ParameterEditorSpec::Text { multiline: false },
            }])
            .build();
        let parameters = ValidatedNodeConfig::from_analysis(
            &protocol,
            BTreeMap::from([(
                ParameterKey::new("target").unwrap(),
                serde_json::json!("functions/raw-string"),
            )]),
            &BTreeMap::new(),
        )
        .unwrap();

        assert_eq!(
            parameters.string(&ParameterKey::new("target").unwrap()),
            Some("functions/raw-string")
        );
        assert_eq!(prepared_call_target(&parameters), None);
    }

    fn node_id(value: u128) -> NodeId {
        NodeId::from_uuid(Uuid::from_u128(value))
    }

    fn node(value: u128) -> FlowNode {
        FlowNode::Node(node_id(value))
    }

    #[test]
    fn structural_port_requirements_emit_port_key_and_direction_facts() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let mut protocol = registry
            .get(&NodeTypeId::new("yssbi.control.branch").unwrap())
            .unwrap()
            .protocol()
            .clone();
        let ports = protocol
            .interface
            .ports
            .iter()
            .filter(|port| !matches!(port.key.as_str(), "condition" | "true"))
            .cloned()
            .collect();
        protocol.interface = NodeInterfaceProtocol::new(ports, vec![], vec![]).unwrap();

        let issues = validate_structural_contract(
            node_id(1),
            StructuralNodeRole::Branch,
            &protocol,
            &BTreeMap::new(),
        );

        assert!(issues.iter().any(|issue| {
            issue.diagnostic
                == CompilerDiagnostic::ControlDataPortRequired {
                    port_key: "condition".into(),
                    expected_direction: "input".into(),
                }
        }));
        assert!(issues.iter().any(|issue| {
            issue.diagnostic
                == CompilerDiagnostic::ControlControlPortRequired {
                    port_key: "true".into(),
                    expected_direction: "output".into(),
                }
        }));
    }

    #[test]
    fn duplicate_call_locator_emits_complete_function_parameter_and_port_facts() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let protocol = registry
            .get(&NodeTypeId::new("yssbi.project.function.call").unwrap())
            .unwrap()
            .protocol();
        let call_id = node_id(10);
        let function_path = GraphResourcePath("functions/customer".into());
        let parameter_id = crate::node_system::document::FunctionParameterId("customer_id".into());
        let first = PortAddress::instance(
            call_id,
            PortKey::new("arguments").unwrap(),
            PortInstanceId::from_uuid(Uuid::from_u128(100)),
        );
        let duplicate = PortAddress::instance(
            call_id,
            PortKey::new("arguments").unwrap(),
            PortInstanceId::from_uuid(Uuid::from_u128(101)),
        );
        let locator = DynamicMemberLocator::FunctionParameter {
            function: function_path.clone(),
            parameter: parameter_id.clone(),
        };
        let parameters = ValidatedNodeConfig::empty();
        let nodes = BTreeMap::from([(
            call_id,
            ControlNode {
                node_id: call_id,
                role: Some(StructuralNodeRole::Call),
                protocol,
                parameters: &parameters,
                ports: Box::from([first.clone(), duplicate.clone()]),
                values: BTreeMap::from([
                    (first.clone(), ValueRef::new(1)),
                    (duplicate.clone(), ValueRef::new(2)),
                ]),
                dynamic_members: BTreeMap::from([
                    (first, locator.clone()),
                    (duplicate.clone(), locator),
                ]),
                operation: None,
            },
        )]);
        let function_abis = BTreeMap::new();
        let builder = RegionBuilder::new(nodes, Vec::new(), &function_abis).unwrap();

        let issue = builder
            .call_members(call_id, &function_path, PortDirection::Input)
            .expect_err("duplicate function locator must fail");

        assert_eq!(
            issue.diagnostic,
            CompilerDiagnostic::ControlCallLocatorDuplicate {
                function_path: function_path.0,
                parameter_id: parameter_id.0,
                port: duplicate.to_string().into(),
            }
        );
    }

    #[test]
    fn branch_postdom_ignores_sequence_outputs_the_walker_cannot_reach() {
        let registry =
            std::sync::Arc::unwrap_or_clone(build_builtin_node_system().unwrap().registry);
        let branch_protocol = registry
            .get(&NodeTypeId::new("yssbi.control.branch").unwrap())
            .unwrap()
            .protocol();
        let base_sequence_protocol = registry
            .get(&NodeTypeId::new("yssbi.control.sequence").unwrap())
            .unwrap()
            .protocol();
        let mut sequence_protocol = base_sequence_protocol.clone();
        let mut ports = sequence_protocol.interface.ports.to_vec();
        let mut extra_output = ports
            .iter()
            .find(|port| port.key.as_str() == "then")
            .unwrap()
            .clone();
        extra_output.key = PortKey::new("zzz").unwrap();
        ports.push(extra_output);
        sequence_protocol.interface = NodeInterfaceProtocol::new(ports, vec![], vec![]).unwrap();

        let branch = node_id(1);
        let sequence = node_id(2);
        let merge = node_id(3);
        let parameters = ValidatedNodeConfig::empty();
        let nodes = BTreeMap::from([
            (
                branch,
                ControlNode {
                    node_id: branch,
                    role: Some(StructuralNodeRole::Branch),
                    protocol: branch_protocol,
                    parameters: &parameters,
                    ports: Box::from([
                        PortAddress::declared(branch, PortKey::new("true").unwrap()),
                        PortAddress::declared(branch, PortKey::new("false").unwrap()),
                    ]),
                    values: BTreeMap::new(),
                    dynamic_members: BTreeMap::new(),
                    operation: None,
                },
            ),
            (
                sequence,
                ControlNode {
                    node_id: sequence,
                    role: Some(StructuralNodeRole::Sequence),
                    protocol: &sequence_protocol,
                    parameters: &parameters,
                    ports: Box::from([
                        PortAddress::declared(sequence, PortKey::new("then").unwrap()),
                        PortAddress::declared(sequence, PortKey::new("zzz").unwrap()),
                    ]),
                    values: BTreeMap::new(),
                    dynamic_members: BTreeMap::new(),
                    operation: None,
                },
            ),
            (
                merge,
                ControlNode {
                    node_id: merge,
                    role: Some(StructuralNodeRole::Sequence),
                    protocol: base_sequence_protocol,
                    parameters: &parameters,
                    ports: Box::from([PortAddress::declared(merge, PortKey::new("then").unwrap())]),
                    values: BTreeMap::new(),
                    dynamic_members: BTreeMap::new(),
                    operation: None,
                },
            ),
        ]);
        let edges = vec![
            ControlEdge {
                source: PortAddress::declared(branch, PortKey::new("true").unwrap()),
                target: PortAddress::declared(sequence, PortKey::new("enter").unwrap()),
            },
            ControlEdge {
                source: PortAddress::declared(sequence, PortKey::new("zzz").unwrap()),
                target: PortAddress::declared(merge, PortKey::new("enter").unwrap()),
            },
            ControlEdge {
                source: PortAddress::declared(branch, PortKey::new("false").unwrap()),
                target: PortAddress::declared(merge, PortKey::new("enter").unwrap()),
            },
        ];
        let function_abis = BTreeMap::new();
        let builder = RegionBuilder::new(nodes, edges, &function_abis).unwrap();

        assert_eq!(builder.branch_continuation(branch, None).unwrap(), None);
    }

    #[test]
    fn post_dominator_fixed_point_is_bounded_for_a_cycle_without_exit() {
        let a = node(1);
        let b = node(2);
        let graph = BTreeMap::from([
            (FlowNode::Exit, BTreeSet::new()),
            (a, BTreeSet::from([b])),
            (b, BTreeSet::from([a])),
        ]);

        assert_eq!(post_dominators(&graph), None);
    }

    #[test]
    fn incomparable_common_post_dominators_are_ambiguous_without_id_tiebreaking() {
        let left = node(1);
        let right = node(2);
        let first = node(3);
        let second = node(4);
        let post_dominators = BTreeMap::from([
            (left, BTreeSet::from([left, first, second, FlowNode::Exit])),
            (
                right,
                BTreeSet::from([right, first, second, FlowNode::Exit]),
            ),
            (first, BTreeSet::from([first, FlowNode::Exit])),
            (second, BTreeSet::from([second, FlowNode::Exit])),
            (FlowNode::Exit, BTreeSet::from([FlowNode::Exit])),
        ]);

        let branch_id = NodeId::from_uuid(Uuid::from_u128(9));
        let error = resolve_common_post_dominator(branch_id, &post_dominators, left, right)
            .expect_err("incomparable candidates must block");
        assert_eq!(
            error.diagnostic,
            CompilerDiagnostic::ControlBranchContinuationAmbiguous {}
        );
        assert_eq!(error.node_id, Some(branch_id));
    }
}
