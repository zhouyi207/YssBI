use super::*;

struct ResolvedNode<'a> {
    registry: RegistryNode<'a>,
    parameters: BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
    instance_title: Option<Box<str>>,
    prepared_nominal: BTreeMap<crate::node_system::protocol::ParameterKey, PreparedNominalValue>,
    ports: BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
    port_sequence: Vec<PortAddress>,
}

pub(super) struct AnalysisState<'a> {
    document: &'a GraphDocument,
    graph_path: GraphResourcePath,
    pub(super) basis: CompilationBasis<GraphRevision>,
    nodes: BTreeMap<NodeId, ResolvedNode<'a>>,
    pub(super) diagnostics: Vec<NodeDiagnostic<NodeId, PortAddress, ConnectionId, Box<str>>>,
    type_facts: BTreeMap<PortAddress, TypeExpr>,
    schema_facts: BTreeMap<PortAddress, crate::node_system::protocol::SchemaExpr>,
    resolved_schema_facts: BTreeMap<PortAddress, crate::node_system::protocol::ResolvedSchemaFact>,
    projection_only_ports: BTreeSet<PortAddress>,
    interface_projections: BTreeMap<NodeId, ValidatedNodeInterfaceProjection>,
    pub(super) decoded_literals: BTreeMap<PortAddress, crate::node_system::protocol::TypedValue>,
}

impl<'a> AnalysisState<'a> {
    pub(super) fn new(
        document: &'a GraphDocument,
        graph_path: GraphResourcePath,
        basis: CompilationBasis<GraphRevision>,
    ) -> Self {
        Self {
            document,
            graph_path,
            basis,
            nodes: BTreeMap::new(),
            diagnostics: Vec::new(),
            type_facts: BTreeMap::new(),
            schema_facts: BTreeMap::new(),
            resolved_schema_facts: BTreeMap::new(),
            projection_only_ports: BTreeSet::new(),
            interface_projections: BTreeMap::new(),
            decoded_literals: BTreeMap::new(),
        }
    }

    pub(super) fn analyze<R: CompilerRegistry>(
        &mut self,
        registry: &'a R,
        schema_resolvers: &SchemaResolverSet,
        interface_resolvers: &InterfaceResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
        cancellation: &CompileCancellationToken,
    ) -> Result<(), CompileCancelled> {
        let empty_schemas = BTreeMap::new();
        let mut deferred_nodes = BTreeSet::new();
        for (&node_id, node) in &self.document.nodes {
            cancellation.checkpoint()?;
            if node.id != node_id {
                self.push(
                    CompilerDiagnostic::DocumentNodeIdMismatch {
                        expected_id: node_id.to_string().into(),
                        actual_id: node.id.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
            }
            let Some(resolved) = registry.resolve(&node.node_type) else {
                self.push(
                    CompilerDiagnostic::NodeUnknown {
                        node_type: node.node_type.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
                continue;
            };
            let path_scope = if self.graph_path.0.starts_with("events/") {
                crate::node_system::protocol::NodeScope::Event
            } else if self.graph_path.0.starts_with("functions/") {
                crate::node_system::protocol::NodeScope::Function
            } else {
                crate::node_system::protocol::NodeScope::Any
            };
            if resolved.protocol.scope != crate::node_system::protocol::NodeScope::Any
                && path_scope != crate::node_system::protocol::NodeScope::Any
                && resolved.protocol.scope != path_scope
            {
                self.push(
                    CompilerDiagnostic::NodeScopeMismatch {
                        expected_scope: node_scope_name(path_scope).into(),
                        actual_scope: node_scope_name(resolved.protocol.scope).into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
            }
            if resolved.protocol.type_id != node.node_type {
                self.push(
                    CompilerDiagnostic::RegistryTypeMismatch {
                        expected_type: node.node_type.to_string().into(),
                        actual_type: resolved.protocol.type_id.to_string().into(),
                    },
                    DiagnosticLocation::Node(node_id),
                );
                continue;
            }
            let (parameters, prepared_nominal) =
                self.normalize_parameters(node_id, resolved.protocol, registry);
            let instance_title =
                self.resolve_instance_title(node_id, resolved.protocol, &parameters, resources);
            self.validate_binding_templates(node_id, resolved.protocol);
            let provisional_diagnostic_start = self.diagnostics.len();
            let (mut ports, port_sequence, deferred_for_schema) = self.resolve_ports(
                node_id,
                resolved.protocol,
                &empty_schemas,
                resources,
                interface_resolvers,
            );
            self.refine_resource_bound_port_types(
                resolved.protocol,
                &parameters,
                resources,
                &mut ports,
            );
            if deferred_for_schema {
                self.diagnostics.truncate(provisional_diagnostic_start);
                deferred_nodes.insert(node_id);
            }
            self.nodes.insert(
                node_id,
                ResolvedNode {
                    registry: resolved,
                    parameters,
                    instance_title,
                    prepared_nominal,
                    ports,
                    port_sequence,
                },
            );
        }
        cancellation.checkpoint()?;
        let (_, preliminary_schemas, _) = self.resolve_schema_facts(schema_resolvers, resources);
        self.complete_schema_dependent_interfaces(
            &deferred_nodes,
            &preliminary_schemas,
            resources,
            interface_resolvers,
        );
        cancellation.checkpoint()?;
        self.validate_function_abi_contract(resources);
        self.validate_call_abi_contract(resources);
        self.validate_structural_control();
        cancellation.checkpoint()?;
        self.validate_connections();
        cancellation.checkpoint()?;
        self.validate_input_bindings(registry);
        cancellation.checkpoint()?;
        self.validate_value_cycles();
        cancellation.checkpoint()?;
        self.analyze_types(registry);
        cancellation.checkpoint()?;
        self.analyze_schemas(schema_resolvers, resources);
        cancellation.checkpoint()?;
        self.diagnostics.sort_by(compare_diagnostics);
        Ok(())
    }

    fn resolve_instance_title(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        parameters: &BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
        resources: &mut dyn AnalysisResourceResolver,
    ) -> Option<Box<str>> {
        let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } =
            &protocol.instance_display
        else {
            return None;
        };
        let resource = parameters
            .get(parameter)
            .and_then(serde_json::Value::as_str);
        let result: Result<Box<str>, (String, bool)> = match (kind, resource) {
            (ResourceDisplayKind::Function, Some(path)) => resources
                .resolve_function(&GraphResourcePath(path.into()))
                .map_err(|error| (error.to_string(), true))
                .and_then(|resolved| {
                    resolved
                        .value
                        .name
                        .filter(|name| !name.trim().is_empty())
                        .map(Into::into)
                        .ok_or_else(|| {
                            (
                                "function resource has no valid display name".to_owned(),
                                false,
                            )
                        })
                }),
            (ResourceDisplayKind::Variable, Some(path)) => path
                .strip_prefix("variables/")
                .ok_or_else(|| (format!("variable resource '{path}' is not canonical"), true))
                .and_then(|id| uuid::Uuid::parse_str(id).map_err(|error| (error.to_string(), true)))
                .and_then(|id| {
                    resources
                        .resolve_variable(&crate::variable::VariableId::from(id))
                        .map_err(|error| (error.to_string(), true))
                })
                .and_then(|resolved| {
                    (!resolved.value.name.trim().is_empty())
                        .then(|| resolved.value.name.as_str().into())
                        .ok_or_else(|| {
                            (
                                "variable resource has no valid display name".to_owned(),
                                false,
                            )
                        })
                }),
            (ResourceDisplayKind::Database, Some(path)) => path
                .strip_prefix("databases/")
                .filter(|id| !id.is_empty())
                .ok_or_else(|| (format!("database resource '{path}' is not canonical"), true))
                .and_then(|id| {
                    resources
                        .resolve_database(id)
                        .map_err(|error| (error.to_string(), true))
                })
                .and_then(|resolved| {
                    resolved
                        .value
                        .name
                        .filter(|name| !name.trim().is_empty())
                        .map(Into::into)
                        .ok_or_else(|| {
                            (
                                "database resource has no valid display name".to_owned(),
                                false,
                            )
                        })
                }),
            (_, None) => Err((
                format!(
                    "resource display parameter '{}' is missing",
                    parameter.as_str()
                ),
                true,
            )),
        };
        match result {
            Ok(title) => Some(title),
            Err((reason, semantic_failure)) => {
                let diagnostic = if semantic_failure {
                    CompilerDiagnostic::resource_resolution_failed(
                        resource.unwrap_or(parameter.as_str()),
                        reason,
                    )
                } else {
                    CompilerDiagnostic::ResourceDisplayNameUnavailable {
                        resource_key: resource.unwrap_or(parameter.as_str()).into(),
                        reason: reason.into(),
                    }
                };
                self.push(diagnostic, DiagnosticLocation::Node(node_id));
                None
            }
        }
    }

    fn validate_function_abi_contract(&mut self, resources: &mut dyn AnalysisResourceResolver) {
        if !self.graph_path.0.starts_with("functions/") {
            return;
        }
        let resolved = match resources.resolve_function(&self.graph_path) {
            Ok(resolved) => resolved,
            Err(error) => {
                self.push(
                    CompilerDiagnostic::resource_resolution_failed(
                        error.key().as_str(),
                        error.reason(),
                    ),
                    DiagnosticLocation::Graph,
                );
                return;
            }
        };
        let function = resolved.value.function;
        let expected_parameters = function
            .signature
            .parameters
            .iter()
            .map(|parameter| parameter.id.clone())
            .collect::<BTreeSet<_>>();
        let expected_results = function
            .signature
            .return_type
            .as_ref()
            .map(|_| FunctionParameterId("return".into()))
            .into_iter()
            .collect::<BTreeSet<_>>();
        self.validate_function_abi_role(
            StructuralNodeRole::FunctionEntry,
            "parameters",
            PortDirection::Output,
            &expected_parameters,
        );
        self.validate_function_abi_role(
            StructuralNodeRole::FunctionReturn,
            "results",
            PortDirection::Input,
            &expected_results,
        );
    }

    fn validate_function_abi_role(
        &mut self,
        role: StructuralNodeRole,
        expected_template: &str,
        expected_direction: PortDirection,
        expected_ids: &BTreeSet<FunctionParameterId>,
    ) {
        let nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                (node.registry.structural_role() == Some(role)).then_some(*node_id)
            })
            .collect::<Vec<_>>();
        if nodes.len() != 1 {
            self.push(
                CompilerDiagnostic::FunctionAbiManagedRoleInvalid {
                    expected_role: structural_role_name(role).into(),
                    actual_count: nodes.len().to_string().into(),
                },
                DiagnosticLocation::Graph,
            );
            return;
        }
        let node_id = nodes[0];
        let protocol = self.nodes[&node_id].registry.protocol;
        let mut counts = BTreeMap::<FunctionParameterId, usize>::new();
        let bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| address.node_id == node_id)
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();
        for (address, binding) in bindings {
            let origin = match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => origin,
                DynamicPortBinding::UserCreated { .. } => continue,
            };
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                self.push(
                    CompilerDiagnostic::FunctionAbiLocatorInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            };
            let template = port_template(&address);
            let spec = protocol
                .interface
                .ports
                .iter()
                .find(|spec| &spec.key == template);
            if template.as_str() != expected_template
                || spec.is_none_or(|spec| {
                    spec.kind != PortKind::Data || spec.direction != expected_direction
                })
            {
                self.push(
                    CompilerDiagnostic::FunctionAbiEndpointInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if function != self.graph_path {
                self.push(
                    CompilerDiagnostic::FunctionAbiLocatorTargetMismatch {
                        function_path: function.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    CompilerDiagnostic::FunctionAbiMemberUnexpected {
                        field_name: parameter.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            *counts.entry(parameter).or_default() += 1;
        }
        for expected in expected_ids {
            match counts.get(expected).copied().unwrap_or(0) {
                0 => self.push(
                    CompilerDiagnostic::FunctionAbiMemberMissing {
                        field_name: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
                1 => {}
                _ => self.push(
                    CompilerDiagnostic::FunctionAbiMemberDuplicate {
                        field_name: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
            }
        }
    }

    fn validate_call_abi_contract(&mut self, resources: &mut dyn AnalysisResourceResolver) {
        let call_nodes = self
            .nodes
            .iter()
            .filter_map(|(node_id, node)| {
                (node.registry.structural_role() == Some(StructuralNodeRole::Call))
                    .then_some(*node_id)
            })
            .collect::<Vec<_>>();
        for node_id in call_nodes {
            let Some(target) = function_target(&self.nodes[&node_id].parameters) else {
                continue;
            };
            let target = GraphResourcePath(target.into());
            let resolved = match resources.resolve_function(&target) {
                Ok(resolved) => resolved,
                Err(error) => {
                    self.push(
                        CompilerDiagnostic::resource_resolution_failed(
                            error.key().as_str(),
                            error.reason(),
                        ),
                        DiagnosticLocation::Node(node_id),
                    );
                    continue;
                }
            };
            let function = resolved.value.function;
            let expected_arguments = function
                .signature
                .parameters
                .iter()
                .map(|parameter| parameter.id.clone())
                .collect::<BTreeSet<_>>();
            let expected_results = function
                .signature
                .return_type
                .as_ref()
                .map(|_| FunctionParameterId("return".into()))
                .into_iter()
                .collect::<BTreeSet<_>>();
            self.validate_call_abi_role(
                node_id,
                &target,
                "arguments",
                PortDirection::Input,
                &expected_arguments,
            );
            self.validate_call_abi_role(
                node_id,
                &target,
                "results",
                PortDirection::Output,
                &expected_results,
            );
        }
    }

    fn validate_call_abi_role(
        &mut self,
        node_id: NodeId,
        target: &GraphResourcePath,
        expected_template: &str,
        expected_direction: PortDirection,
        expected_ids: &BTreeSet<FunctionParameterId>,
    ) {
        let protocol = self.nodes[&node_id].registry.protocol.clone();
        let bindings = self
            .document
            .port_bindings
            .iter()
            .filter(|(address, _)| {
                address.node_id == node_id && port_template(address).as_str() == expected_template
            })
            .map(|(address, binding)| (address.clone(), binding.clone()))
            .collect::<Vec<_>>();
        let mut member_ports = BTreeMap::<FunctionParameterId, Vec<PortAddress>>::new();
        for (address, binding) in bindings {
            let origin = match binding {
                DynamicPortBinding::Resolved { origin, .. }
                | DynamicPortBinding::Orphan { origin, .. } => origin,
                DynamicPortBinding::UserCreated { .. } => {
                    self.push(
                        CompilerDiagnostic::ControlCallLocatorInvalid {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address),
                    );
                    continue;
                }
            };
            let spec = protocol
                .interface
                .ports
                .iter()
                .find(|spec| spec.key.as_str() == expected_template);
            if spec.is_none_or(|spec| {
                spec.kind != PortKind::Data || spec.direction != expected_direction
            }) {
                self.push(
                    CompilerDiagnostic::ControlCallEndpointInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            let DynamicMemberLocator::FunctionParameter {
                function,
                parameter,
            } = origin
            else {
                self.push(
                    CompilerDiagnostic::ControlCallLocatorInvalid {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            };
            if &function != target {
                self.push(
                    CompilerDiagnostic::ControlCallLocatorTargetMismatch {
                        function_path: function.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            if !expected_ids.contains(&parameter) {
                self.push(
                    CompilerDiagnostic::ControlCallMemberUnexpected {
                        member_role: call_member_role(expected_template).into(),
                        member_id: parameter.0.clone(),
                    },
                    DiagnosticLocation::Port(address),
                );
                continue;
            }
            member_ports.entry(parameter).or_default().push(address);
        }
        for expected in expected_ids {
            match member_ports
                .get(expected)
                .map(Vec::as_slice)
                .unwrap_or_default()
            {
                [] => self.push(
                    CompilerDiagnostic::ControlCallMemberMissing {
                        member_role: call_member_role(expected_template).into(),
                        member_id: expected.0.clone(),
                    },
                    DiagnosticLocation::Node(node_id),
                ),
                [_] => {}
                [_, duplicate, ..] => self.push(
                    CompilerDiagnostic::ControlCallLocatorDuplicate {
                        function_path: target.0.clone(),
                        parameter_id: expected.0.clone(),
                        port: duplicate.to_string().into(),
                    },
                    DiagnosticLocation::Port((*duplicate).clone()),
                ),
            }
        }
    }

    fn validate_structural_control(&mut self) {
        use crate::node_system::protocol::ManagedNodeRole;
        for role in [
            ManagedNodeRole::EventBegin,
            ManagedNodeRole::FunctionEntry,
            ManagedNodeRole::FunctionReturn,
        ] {
            let nodes = self
                .nodes
                .iter()
                .filter_map(|(node_id, node)| {
                    (node.registry.protocol.managed_role == Some(role)).then_some(*node_id)
                })
                .collect::<Vec<_>>();
            if nodes.len() > 1 {
                for node_id in nodes {
                    self.push(
                        CompilerDiagnostic::NodeManagedSingleton {
                            managed_role: managed_node_role_name(Some(role)).into(),
                        },
                        DiagnosticLocation::Node(node_id),
                    );
                }
            }
        }
        let issues = self
            .nodes
            .iter()
            .flat_map(|(&node_id, node)| {
                node.registry
                    .structural_role()
                    .into_iter()
                    .flat_map(move |role| {
                        validate_structural_contract(
                            node_id,
                            role,
                            node.registry.protocol,
                            &node.parameters,
                        )
                    })
            })
            .collect::<Vec<_>>();
        for issue in issues {
            self.push(
                issue.diagnostic,
                issue
                    .node_id
                    .map(DiagnosticLocation::Node)
                    .unwrap_or(DiagnosticLocation::Graph),
            );
        }
    }

    fn normalize_parameters<R: CompilerRegistry>(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        registry: &R,
    ) -> (
        BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
        BTreeMap<crate::node_system::protocol::ParameterKey, PreparedNominalValue>,
    ) {
        let supplied = &self.document.nodes[&node_id].parameters;
        let mut values = supplied.clone();
        for spec in protocol.parameters.parameters.iter() {
            if !values.contains_key(&spec.key)
                && let Some(default) = &spec.default_value
            {
                values.insert(spec.key.clone(), protocol_value_to_json(&default.value));
            }
        }
        let validation =
            validate_and_prepare_parameter_values(protocol, &values, |type_id, value| {
                registry.prepare_nominal_parameter(type_id, value)
            });
        for issue in validation.issues {
            let diagnostic = match issue.kind {
                ParameterIssueKind::Unknown => CompilerDiagnostic::ParameterUnknown {
                    parameter_key: issue.key.to_string().into(),
                },
                ParameterIssueKind::Required => CompilerDiagnostic::ParameterRequired {
                    parameter_key: issue.key.to_string().into(),
                },
                ParameterIssueKind::InvalidType
                | ParameterIssueKind::Constraint
                | ParameterIssueKind::InvalidNominal(_)
                | ParameterIssueKind::InvalidResourceId => CompilerDiagnostic::ParameterInvalid {
                    parameter_key: issue.key.to_string().into(),
                },
            };
            self.push(
                diagnostic,
                DiagnosticLocation::Parameter {
                    node_id,
                    key: issue.key,
                },
            );
        }
        let known = protocol
            .parameters
            .parameters
            .iter()
            .map(|spec| &spec.key)
            .collect::<BTreeSet<_>>();
        let normalized = values
            .into_iter()
            .filter(|(key, _)| known.contains(key))
            .collect();
        (normalized, validation.prepared_nominal)
    }

    fn refine_resource_bound_port_types(
        &mut self,
        protocol: &NodeProtocol,
        parameters: &BTreeMap<crate::node_system::protocol::ParameterKey, serde_json::Value>,
        resources: &mut dyn AnalysisResourceResolver,
        ports: &mut BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
    ) {
        let NodeInstanceDisplaySpec::ResourceParameter { parameter, kind } =
            &protocol.instance_display
        else {
            return;
        };
        let Some(path) = parameters
            .get(parameter)
            .and_then(serde_json::Value::as_str)
        else {
            return;
        };
        let value_type = match kind {
            ResourceDisplayKind::Variable => path
                .strip_prefix("variables/")
                .and_then(|id| uuid::Uuid::parse_str(id).ok())
                .and_then(|id| {
                    resources
                        .resolve_variable(&crate::variable::VariableId::from(id))
                        .ok()
                })
                .and_then(|resolved| {
                    crate::node_system::compatibility::data_type_to_type_expr(
                        &resolved.value.data_type,
                    )
                    .ok()
                }),
            ResourceDisplayKind::Function | ResourceDisplayKind::Database => None,
        };
        let Some(value_type) = value_type else {
            return;
        };
        for port in ports
            .values_mut()
            .filter(|port| port.kind == PortKind::Data)
        {
            if matches!(port.value_type, TypeExpr::Generic(_) | TypeExpr::Unknown) {
                port.value_type = value_type.clone();
            }
        }
    }

    fn resolve_ports(
        &mut self,
        node_id: NodeId,
        protocol: &NodeProtocol,
        resolved_schemas: &BTreeMap<PortAddress, ResolvedSchemaFact>,
        resources: &mut dyn AnalysisResourceResolver,
        resolvers: &InterfaceResolverSet,
    ) -> (
        BTreeMap<PortAddress, ResolvedPort<PortAddress>>,
        Vec<PortAddress>,
        bool,
    ) {
        let DynamicInterfaceResolution {
            interface,
            projected_bindings,
            available_members,
            diagnostics,
            deferred_for_schema,
        } = materialize_dynamic_interface_with_resources(
            &self.basis,
            node_id,
            protocol,
            self.document,
            resolved_schemas,
            resources,
            resolvers,
        );

        self.projection_only_ports.extend(
            available_members
                .iter()
                .filter(|member| member.bound_address().is_none())
                .map(|member| member.projection_address().clone()),
        );
        self.diagnostics.extend(diagnostics);
        self.interface_projections.insert(
            node_id,
            ValidatedNodeInterfaceProjection {
                projected_bindings,
                available_members,
            },
        );
        let port_sequence = interface
            .ports
            .iter()
            .map(|port| port.address.clone())
            .collect();
        let ports = interface
            .ports
            .into_vec()
            .into_iter()
            .map(|port| (port.address.clone(), port))
            .collect();
        (ports, port_sequence, deferred_for_schema)
    }

    fn complete_schema_dependent_interfaces(
        &mut self,
        deferred_nodes: &BTreeSet<NodeId>,
        resolved_schemas: &BTreeMap<PortAddress, ResolvedSchemaFact>,
        resources: &mut dyn AnalysisResourceResolver,
        resolvers: &InterfaceResolverSet,
    ) {
        for &node_id in deferred_nodes {
            let Some(protocol) = self.nodes.get(&node_id).map(|node| node.registry.protocol) else {
                continue;
            };
            self.projection_only_ports
                .retain(|address| address.node_id != node_id);
            self.interface_projections.remove(&node_id);
            let (ports, port_sequence, deferred_for_schema) =
                self.resolve_ports(node_id, protocol, resolved_schemas, resources, resolvers);
            if let Some(node) = self.nodes.get_mut(&node_id) {
                node.ports = ports;
                node.port_sequence = port_sequence;
            }
            if deferred_for_schema {
                self.push(
                    CompilerDiagnostic::InterfaceSchemaDependencyUnresolved {},
                    DiagnosticLocation::Node(node_id),
                );
            }
        }
    }

    fn validate_binding_templates(&mut self, node_id: NodeId, protocol: &NodeProtocol) {
        for address in self
            .document
            .port_bindings
            .keys()
            .filter(|address| address.node_id == node_id)
        {
            let PortRef::Instance { template, .. } = &address.port else {
                self.push(
                    CompilerDiagnostic::PortBindingNotInstance {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
                continue;
            };
            let Some(spec) = protocol
                .interface
                .ports
                .iter()
                .find(|port| &port.key == template)
            else {
                self.push(
                    CompilerDiagnostic::PortUnknown {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
                continue;
            };
            if spec.instances == PortInstances::Declared {
                self.push(
                    CompilerDiagnostic::PortInstanceNotAllowed {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
        }
    }

    fn validate_connections(&mut self) {
        let mut counts: BTreeMap<PortAddress, usize> = BTreeMap::new();
        for (&connection_id, connection) in &self.document.connections {
            if connection.id != connection_id {
                self.push(
                    CompilerDiagnostic::DocumentConnectionIdMismatch {
                        expected_id: connection_id.to_string().into(),
                        actual_id: connection.id.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            let output = self.lookup_document_port(&connection.output).cloned();
            let input = self.lookup_document_port(&connection.input).cloned();
            if output.is_none() {
                self.push(
                    CompilerDiagnostic::PortUnknown {
                        port: connection.output.to_string().into(),
                    },
                    DiagnosticLocation::Port(connection.output.clone()),
                );
            }
            if input.is_none() {
                self.push(
                    CompilerDiagnostic::PortUnknown {
                        port: connection.input.to_string().into(),
                    },
                    DiagnosticLocation::Port(connection.input.clone()),
                );
            }
            let (Some(output), Some(input)) = (output, input) else {
                continue;
            };
            if output.direction != PortDirection::Output {
                self.push(
                    CompilerDiagnostic::ConnectionOutputDirection {
                        port: connection.output.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if input.direction != PortDirection::Input {
                self.push(
                    CompilerDiagnostic::ConnectionInputDirection {
                        port: connection.input.to_string().into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if output.kind != input.kind {
                self.push(
                    CompilerDiagnostic::ConnectionKindMismatch {
                        source_kind: port_kind_name(output.kind).into(),
                        target_kind: port_kind_name(input.kind).into(),
                    },
                    DiagnosticLocation::Connection(connection_id),
                );
            }
            if let Some(spec) = self.port_spec(&connection.input, &input.template) {
                match spec.connections {
                    ConnectionsPerPort::Multiple { ordered: true, .. }
                        if connection.order.is_none() =>
                    {
                        self.push(
                            CompilerDiagnostic::ConnectionOrderRequired {
                                port: connection.input.to_string().into(),
                            },
                            DiagnosticLocation::Connection(connection_id),
                        );
                    }
                    ConnectionsPerPort::Single
                    | ConnectionsPerPort::Multiple { ordered: false, .. }
                        if connection.order.is_some() =>
                    {
                        self.push(
                            CompilerDiagnostic::ConnectionOrderForbidden {
                                port: connection.input.to_string().into(),
                            },
                            DiagnosticLocation::Connection(connection_id),
                        );
                    }
                    _ => {}
                }
            }
            *counts.entry(connection.output.clone()).or_default() += 1;
            *counts.entry(connection.input.clone()).or_default() += 1;
        }
        for (address, count) in counts {
            let Some(port) = self.lookup_document_port(&address) else {
                continue;
            };
            let spec = self.port_spec(&address, &port.template);
            if let Some(spec) = spec {
                let exceeded = match spec.connections {
                    ConnectionsPerPort::Single => count > 1,
                    ConnectionsPerPort::Multiple { max, .. } => {
                        max.is_some_and(|max| count > max as usize)
                    }
                };
                if exceeded {
                    self.push(
                        CompilerDiagnostic::ConnectionLimit {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
                    );
                }
            }
        }
    }

    fn validate_input_bindings<R: CompilerRegistry>(&mut self, registry: &R) {
        let addresses: Vec<_> = self
            .nodes
            .values()
            .flat_map(|node| node.ports.keys())
            .filter(|address| !self.projection_only_ports.contains(*address))
            .cloned()
            .collect();
        for address in addresses {
            let port = self
                .lookup_document_port(&address)
                .cloned()
                .expect("address came from resolved ports");
            if port.direction != PortDirection::Input {
                if self.document.input_states.contains_key(&address) {
                    self.push(
                        CompilerDiagnostic::InputNotInput {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
                    );
                }
                continue;
            }
            let connections = self
                .document
                .connections
                .values()
                .filter(|connection| connection.input == address)
                .count();
            let literal = self
                .document
                .input_states
                .get(&address)
                .and_then(|state| state.literal_override.as_ref());
            let spec = self
                .port_spec(&address, &port.template)
                .cloned()
                .expect("resolved port has protocol spec");
            if let Some(literal) = literal {
                match crate::node_system::protocol::validate_typed_literal(
                    literal,
                    &spec.value_type,
                    &CompilerNominalValidator(registry),
                ) {
                    Ok(decoded) => {
                        self.decoded_literals.insert(address.clone(), decoded);
                    }
                    Err(_) => self.push(
                        CompilerDiagnostic::InputLiteralInvalid {
                            port: address.to_string().into(),
                        },
                        DiagnosticLocation::Port(address.clone()),
                    ),
                }
            }
            if literal.is_some() && connections != 0 {
                self.push(
                    CompilerDiagnostic::InputConflictingBindings {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
            if literal.is_some()
                && spec
                    .input_binding
                    .as_ref()
                    .is_none_or(|binding| binding.literal_policy == LiteralPolicy::Forbidden)
            {
                self.push(
                    CompilerDiagnostic::InputLiteralForbidden {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
            let has_default = spec
                .input_binding
                .as_ref()
                .is_some_and(|binding| binding.default_value.is_some());
            if port.kind == PortKind::Data && connections == 0 && literal.is_none() && !has_default
            {
                self.push(
                    CompilerDiagnostic::InputUnbound {
                        port: address.to_string().into(),
                    },
                    DiagnosticLocation::Port(address.clone()),
                );
            }
        }
        let stale: Vec<_> = self
            .document
            .input_states
            .keys()
            .filter(|address| self.lookup_document_port(address).is_none())
            .cloned()
            .collect();
        for address in stale {
            self.push(
                CompilerDiagnostic::InputUnknownPort {
                    port: address.to_string().into(),
                },
                DiagnosticLocation::Port(address.clone()),
            );
        }
    }

    fn validate_value_cycles(&mut self) {
        let edges = self
            .document
            .connections
            .values()
            .filter(|connection| {
                let Some(output) = self.lookup_document_port(&connection.output) else {
                    return false;
                };
                let Some(input) = self.lookup_document_port(&connection.input) else {
                    return false;
                };
                let is_loop_condition_feedback = connection.output.node_id
                    == connection.input.node_id
                    && output.template.as_str() == "body_input"
                    && input.template.as_str() == "condition"
                    && self
                        .nodes
                        .get(&connection.output.node_id)
                        .is_some_and(|node| {
                            node.registry.structural_role() == Some(StructuralNodeRole::Loop)
                        });
                output.kind == PortKind::Data
                    && input.kind == PortKind::Data
                    && !is_loop_condition_feedback
            })
            .map(|connection| {
                (
                    connection.id,
                    connection.output.node_id,
                    connection.input.node_id,
                )
            })
            .collect::<Vec<_>>();
        for connection_id in cyclic_value_dependencies(&edges) {
            self.push(
                CompilerDiagnostic::DependencyValueCycle {},
                DiagnosticLocation::Connection(connection_id),
            );
        }
    }

    fn analyze_types<R: CompilerRegistry>(&mut self, registry: &R) {
        let mut graph = TypeConstraintGraph::new();
        for (&node_id, node) in &self.nodes {
            graph.add_node(
                node_id,
                node.registry.protocol,
                node.ports
                    .values()
                    .map(|port| (&port.address, &port.value_type)),
            );
        }
        for connection in self.document.connections.values() {
            let is_value = self
                .lookup_document_port(&connection.output)
                .is_some_and(|port| port.kind == PortKind::Data)
                && self
                    .lookup_document_port(&connection.input)
                    .is_some_and(|port| port.kind == PortKind::Data);
            if is_value {
                graph.add_connection(connection.id, &connection.output, &connection.input);
            }
        }
        for (address, literal) in &self.decoded_literals {
            graph.add_literal(address, &literal.value_type);
        }
        let (facts, issues) = graph.solve(registry);
        self.type_facts = facts;
        for issue in issues {
            self.push(issue.diagnostic, issue.location);
        }
    }

    fn resolve_schema_facts(
        &self,
        resolvers: &SchemaResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
    ) -> (
        BTreeMap<PortAddress, SchemaExpr>,
        BTreeMap<PortAddress, ResolvedSchemaFact>,
        Vec<SchemaAnalysisIssue>,
    ) {
        let mut analyzer = SchemaAnalyzer::new(resolvers);
        for (&node_id, node) in &self.nodes {
            analyzer.add_node(
                node_id,
                node.registry.protocol,
                &node.parameters,
                node.ports.keys().cloned(),
            );
        }
        for connection in self.document.connections.values() {
            if self
                .lookup_document_port(&connection.output)
                .is_some_and(|port| port.kind == PortKind::Data)
                && self
                    .lookup_document_port(&connection.input)
                    .is_some_and(|port| port.kind == PortKind::Data)
            {
                analyzer.add_connection(connection.output.clone(), connection.input.clone());
            }
        }
        analyzer.analyze_with_resources(resources)
    }

    fn analyze_schemas(
        &mut self,
        resolvers: &SchemaResolverSet,
        resources: &mut dyn AnalysisResourceResolver,
    ) {
        let (expressions, facts, issues) = self.resolve_schema_facts(resolvers, resources);
        self.schema_facts = expressions;
        self.resolved_schema_facts = facts;
        for issue in issues {
            self.push(issue.diagnostic, issue.location);
        }
    }

    fn lookup_document_port(&self, address: &PortAddress) -> Option<&ResolvedPort<PortAddress>> {
        if self.projection_only_ports.contains(address) {
            return None;
        }
        self.nodes.get(&address.node_id)?.ports.get(address)
    }
    fn port_spec(
        &self,
        address: &PortAddress,
        key: &crate::node_system::protocol::PortKey,
    ) -> Option<&PortSpec> {
        self.nodes
            .get(&address.node_id)?
            .registry
            .protocol
            .interface
            .ports
            .iter()
            .find(|port| &port.key == key)
    }
    fn push(&mut self, diagnostic: CompilerDiagnostic, location: CompilerDiagnosticLocation) {
        self.diagnostics.push(diagnostic.into_node(location));
    }

    pub(super) fn interface_projection(&self) -> ValidatedInterfaceProjection {
        ValidatedInterfaceProjection {
            basis: self.basis.clone(),
            nodes: self.interface_projections.clone(),
        }
    }

    pub(super) fn prepared_configs(&mut self) -> BTreeMap<NodeId, ValidatedNodeConfig> {
        let attempts = self
            .nodes
            .iter()
            .map(|(&node_id, node)| {
                (
                    node_id,
                    ValidatedNodeConfig::from_analysis(
                        node.registry.protocol,
                        node.parameters.clone(),
                        &node.prepared_nominal,
                    ),
                )
            })
            .collect::<Vec<_>>();
        let mut prepared = BTreeMap::new();
        for (node_id, attempt) in attempts {
            match attempt {
                Ok(config) => {
                    prepared.insert(node_id, config);
                }
                Err(keys) => {
                    for key in keys {
                        self.push(
                            CompilerDiagnostic::ParameterInvalid {
                                parameter_key: key.to_string().into(),
                            },
                            DiagnosticLocation::Parameter { node_id, key },
                        );
                    }
                }
            }
        }
        self.diagnostics.sort_by(compare_diagnostics);
        prepared
    }

    pub(super) fn snapshot(&self) -> CompilerAnalysis {
        let nodes = self
            .nodes
            .iter()
            .map(|(&node_id, node)| AnalyzedNode {
                node_id,
                node_type_id: node.registry.protocol.type_id.clone(),
                protocol_fingerprint: node.registry.protocol_fingerprint.clone(),
                normalized_parameters: node.parameters.clone(),
                instance_title: node.instance_title.clone(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let resolved_interfaces = self
            .nodes
            .iter()
            .map(|(&node_id, node)| ResolvedInterface {
                node_id,
                ports: node
                    .port_sequence
                    .iter()
                    .filter_map(|address| node.ports.get(address).cloned())
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        AnalysisSnapshot {
            basis: self.basis.clone(),
            nodes,
            resolved_interfaces,
            partial_types: self.type_facts.clone(),
            partial_schemas: self.schema_facts.clone(),
            resolved_schemas: self.resolved_schema_facts.clone(),
            diagnostics: self.diagnostics.clone().into_boxed_slice(),
        }
    }

    pub(super) fn semantic_graph(&self) -> CompilerSemanticGraph {
        let nodes = self
            .nodes
            .iter()
            .map(|(&node_id, node)| ValidatedSemanticNode {
                node_id,
                node_type_id: node.registry.protocol.type_id.clone(),
                protocol_fingerprint: node.registry.protocol_fingerprint.clone(),
                normalized_parameters: node.parameters.clone(),
                ports: node
                    .port_sequence
                    .iter()
                    .filter(|address| !self.projection_only_ports.contains(*address))
                    .map(|address| ValidatedSemanticPort {
                        address: address.clone(),
                        resolved_type: self.type_facts.get(address).cloned(),
                        resolved_schema: self.schema_facts.get(address).cloned(),
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let dependencies = self
            .document
            .connections
            .values()
            .map(|connection| {
                let kind = self
                    .lookup_document_port(&connection.output)
                    .map(|port| port.kind)
                    .unwrap_or(PortKind::Data);
                match kind {
                    PortKind::Data => SemanticDependency::Value(ValueEdge {
                        connection_id: connection.id,
                        source: connection.output.clone(),
                        target: connection.input.clone(),
                    }),
                    PortKind::Control => SemanticDependency::Control(ControlEdge {
                        connection_id: connection.id,
                        source_node: connection.output.node_id,
                        source_port: connection.output.clone(),
                        target_node: connection.input.node_id,
                        target_port: connection.input.clone(),
                    }),
                    PortKind::Effect => {
                        SemanticDependency::Effect(crate::node_system::analysis::EffectDependency {
                            predecessor: connection.output.node_id,
                            successor: connection.input.node_id,
                            effect_key: connection.id.to_string().into(),
                        })
                    }
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        ValidatedSemanticGraph {
            basis: self.basis.clone(),
            nodes,
            dependencies,
            resolved_schemas: self.resolved_schema_facts.clone(),
        }
    }
}
