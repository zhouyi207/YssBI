use crate::resolution::resolve_graph_semantics_inner;
use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphFunctionAbi, GraphFunctionParameter,
    GraphFunctionResult, GraphFunctionSemanticFact, GraphPortBacking, GraphPortSemanticFact,
    GraphResolutionOutcome, GraphSemanticCache, GraphSemanticSnapshot, graph_problem,
};
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    DynamicMemberLocator, FunctionParameterId, GraphDocument, GraphResourcePath,
};
use yss_graph_resource_contract::{FunctionSignature, ResourceCatalogSnapshot};
use yss_node_protocol::PortDirection;
use yss_node_registry::{NodeRegistry, StructuralNodeRole};

#[derive(Default)]
pub(crate) struct FunctionResolution {
    pub diagnostics: Vec<GraphDiagnosticFact>,
    pub functions: BTreeMap<GraphResourcePath, GraphFunctionSemanticFact>,
    pub internal_failure: Option<GraphResolutionOutcome>,
}

/// Valid direct call targets in document order. Capture and semantic validation
/// share this parsing; each caller owns its traversal and missing-resource policy.
pub fn direct_function_dependencies<'a>(
    document: &'a GraphDocument,
    registry: &'a NodeRegistry,
) -> impl Iterator<Item = GraphResourcePath> + 'a {
    document.nodes.values().filter_map(|node| {
        let registered = registry.get(&node.node_type)?;
        if !registered
            .structural_role()
            .is_some_and(StructuralNodeRole::calls_function)
        {
            return None;
        }
        GraphResourcePath::new(registered.function_reference(&node.parameters)?).ok()
    })
}

pub(crate) fn resolve(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> FunctionResolution {
    let mut pending = direct_function_dependencies(document, registry)
        .map(|path| (path, false))
        .collect::<Vec<_>>();
    let mut active = BTreeSet::new();
    let mut complete = BTreeSet::new();
    let mut resolution = FunctionResolution::default();
    let diagnostics = &mut resolution.diagnostics;
    while let Some((path, leaving)) = pending.pop() {
        if leaving {
            active.remove(&path);
            complete.insert(path);
            continue;
        }
        if active.contains(&path) {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionDependencyCycle, &path));
            continue;
        }
        if complete.contains(&path) {
            continue;
        }
        let Some(signature) = resources.function_signature(&path) else {
            diagnostics.push(graph_problem(
                GraphDiagnosticKind::ResourceResolutionFailed,
                GraphDiagnosticLocation::Resource(path.as_str().into()),
                [("resource_key", path.as_str().into())],
            ));
            continue;
        };
        let Some(body) = resources.function_document(&path) else {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionBodyUnavailable, &path));
            continue;
        };
        if yss_graph_document_edit::validate_graph_document(body).is_err() {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionBlocked, &path));
            complete.insert(path);
            continue;
        }
        let entries = body
            .nodes
            .values()
            .filter(|node| has_role(registry, &node.node_type, StructuralNodeRole::FunctionEntry))
            .collect::<Vec<_>>();
        let returns = body
            .nodes
            .values()
            .filter(|node| {
                has_role(
                    registry,
                    &node.node_type,
                    StructuralNodeRole::FunctionReturn,
                )
            })
            .collect::<Vec<_>>();
        let mismatched_owner = entries.iter().chain(returns.iter()).any(|node| {
            registry
                .get(&node.node_type)
                .and_then(|registered| registered.function_reference(&node.parameters))
                != Some(path.as_str())
        });
        if entries.len() != 1
            || returns.len() > 1
            || (signature.result().is_some() && returns.len() != 1)
            || mismatched_owner
        {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionAbiMismatch, &path));
        }
        let semantics = resolve_graph_semantics_inner(
            body,
            registry,
            resources,
            &mut GraphSemanticCache::default(),
            &Default::default(),
            &Default::default(),
            false,
        );
        if matches!(
            semantics.outcome(),
            GraphResolutionOutcome::InternalFailure { .. }
        ) {
            // A resolver failure inside a callee remains an internal failure at the root.
            resolution.internal_failure = Some(GraphResolutionOutcome::InternalFailure {
                stage: crate::GraphResolutionStage::Analysis,
                code: "graph.function.resolution_failed".into(),
                node_id: None,
            });
        } else if !crate::function_arguments::permits_unbound_schema(&semantics) {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionBlocked, &path));
        }
        if let Some(abi) = resolve_abi(&path, body, signature, &semantics, registry) {
            resolution.functions.insert(
                path.clone(),
                GraphFunctionSemanticFact {
                    state: if crate::function_arguments::permits_unbound_schema(&semantics)
                        && entries.len() == 1
                        && returns.len() <= 1
                        && !mismatched_owner
                    {
                        crate::GraphFunctionState::Unbound
                    } else {
                        crate::GraphFunctionState::Invalid
                    },
                    abi,
                    semantics,
                },
            );
        } else {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionAbiMismatch, &path));
        }
        active.insert(path.clone());
        pending.push((path, true));
        pending.extend(direct_function_dependencies(body, registry).map(|callee| (callee, false)));
    }
    resolution
}

fn resolve_abi(
    path: &GraphResourcePath,
    document: &GraphDocument,
    signature: &FunctionSignature,
    semantics: &GraphSemanticSnapshot,
    registry: &NodeRegistry,
) -> Option<GraphFunctionAbi> {
    let entry = semantics
        .nodes()
        .iter()
        .find(|node| has_role(registry, &node.node_type, StructuralNodeRole::FunctionEntry))?;
    let mut identities = BTreeSet::new();
    let parameters = signature
        .parameters()
        .iter()
        .map(|parameter| {
            if !identities.insert(parameter.id()) {
                return None;
            }
            let port = function_port(
                document,
                &entry.ports,
                path,
                parameter.id(),
                PortDirection::Output,
            )?;
            let value_type = port.type_state.exact()?.clone();
            if yss_graph_type_mapping::data_type_from_resolved_type(&value_type).as_ref()
                != Some(parameter.data_type())
            {
                return None;
            }
            Some(GraphFunctionParameter {
                id: parameter.id().clone(),
                entry_output: port.address.clone(),
                value_type,
            })
        })
        .collect::<Option<Box<[_]>>>()?;
    let result = match signature.result() {
        Some(expected_type) => {
            let result_id = FunctionParameterId::new("return");
            let return_node = semantics.nodes().iter().find(|node| {
                has_role(
                    registry,
                    &node.node_type,
                    StructuralNodeRole::FunctionReturn,
                )
            })?;
            let port = function_port(
                document,
                &return_node.ports,
                path,
                &result_id,
                PortDirection::Input,
            )?;
            let value_type = port.type_state.exact()?.clone();
            if yss_graph_type_mapping::data_type_from_resolved_type(&value_type).as_ref()
                != Some(expected_type)
            {
                return None;
            }
            Some(GraphFunctionResult {
                id: result_id,
                return_input: port.address.clone(),
                value_type,
            })
        }
        None => None,
    };
    Some(GraphFunctionAbi { parameters, result })
}

fn has_role(
    registry: &NodeRegistry,
    node_type: &yss_node_protocol::NodeTypeId,
    role: StructuralNodeRole,
) -> bool {
    registry
        .get(node_type)
        .is_some_and(|registered| registered.structural_role() == Some(role))
}

fn function_port<'a>(
    document: &GraphDocument,
    ports: &'a [GraphPortSemanticFact],
    function: &GraphResourcePath,
    parameter: &FunctionParameterId,
    direction: PortDirection,
) -> Option<&'a GraphPortSemanticFact> {
    ports.iter().find(|port| {
        if port.orphan || port.direction != direction { return false; }
        let origin = match &port.backing {
            GraphPortBacking::ProjectedDerived { origin } => Some(origin),
            _ => document.port_bindings.get(&port.address).and_then(crate::port_projection::binding_origin),
        };
        matches!(origin, Some(DynamicMemberLocator::FunctionParameter { function: owner, parameter: identity }) if owner == function && identity == parameter)
    })
}

fn problem(kind: GraphDiagnosticKind, path: &GraphResourcePath) -> GraphDiagnosticFact {
    graph_problem(
        kind,
        GraphDiagnosticLocation::Resource(path.as_str().into()),
        [("function", path.as_str().into())],
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use yss_graph_document::{
        DocumentNode, DynamicPortBinding, NodeId, NodePosition, ParameterValues,
    };
    use yss_graph_resource_contract::{FunctionCatalogEntry, FunctionParameterContract};

    fn path(name: &str) -> GraphResourcePath {
        GraphResourcePath::new(format!("functions/{name}.yssbi-function")).unwrap()
    }

    fn node(document: &mut GraphDocument, kind: &str, key: &str, path: &GraphResourcePath) {
        let id = NodeId::new();
        document.nodes.insert(
            id,
            DocumentNode {
                id,
                node_type: kind.parse().unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: ParameterValues::from([(
                    key.parse().unwrap(),
                    serde_json::json!(path.as_str()),
                )]),
                user_label: None,
            },
        );
    }

    fn body(owner: &GraphResourcePath, callees: &[GraphResourcePath]) -> GraphDocument {
        let mut body = GraphDocument::default();
        node(&mut body, "yssbi.project.function.entry", "function", owner);
        for callee in callees {
            node(&mut body, "yssbi.project.function.call", "target", callee);
        }
        body
    }

    fn catalog(
        functions: &[(GraphResourcePath, FunctionSignature, GraphDocument)],
    ) -> ResourceCatalogSnapshot {
        let catalog = ResourceCatalogSnapshot::new(
            functions
                .iter()
                .map(|(path, signature, _)| {
                    (path.clone(), FunctionCatalogEntry::new(signature.clone()))
                })
                .collect(),
            BTreeMap::new(),
        );
        functions.iter().fold(catalog, |catalog, (path, _, body)| {
            catalog.with_function_document(path, body.clone())
        })
    }

    #[test]
    fn reachable_functions_share_one_snapshot_and_abi_uses_signature_identity_order() {
        let registry = yss_node_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let [a, b, c] = [path("A"), path("B"), path("C")];
        let signature = FunctionSignature::new(
            vec![
                FunctionParameterContract::new(
                    FunctionParameterId::new("z"),
                    "Value",
                    yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                ),
                FunctionParameterContract::new(
                    FunctionParameterId::new("a"),
                    "Value",
                    yss_data_contract::ValueType::Scalar(yss_data_contract::SemanticType::Numeric),
                ),
            ],
            None,
        );
        let catalog = catalog(&[
            (a.clone(), signature, body(&a, std::slice::from_ref(&c))),
            (
                b.clone(),
                FunctionSignature::new(vec![], None),
                body(&b, std::slice::from_ref(&c)),
            ),
            (
                c.clone(),
                FunctionSignature::new(vec![], None),
                body(&c, &[]),
            ),
        ]);
        let mut root = GraphDocument::default();
        for path in [&a, &b] {
            node(&mut root, "yssbi.project.function.call", "target", path);
        }
        let resolution = resolve(&root, &registry, &catalog);
        assert!(
            resolution.diagnostics.is_empty(),
            "{:?}",
            resolution.diagnostics
        );
        assert!(resolution.internal_failure.is_none());
        assert_eq!(resolution.functions.len(), 3);
        let selected = root
            .nodes
            .values()
            .find(|node| {
                node.parameters
                    .values()
                    .any(|value| value.as_str() == Some(a.as_str()))
            })
            .unwrap()
            .id;
        let semantics = crate::resolve_graph_semantics(&root, &registry, &catalog);
        assert_eq!(
            semantics.resources_for_nodes(&std::collections::BTreeSet::from([selected])),
            std::collections::BTreeSet::from([a.as_str(), c.as_str()])
        );
        let function = &resolution.functions[&a];
        assert!(function.semantics.ready().is_some());
        assert!(function.semantics.functions().is_empty());
        assert_eq!(
            function
                .abi
                .parameters
                .iter()
                .map(|parameter| parameter.id.as_str())
                .collect::<Vec<_>>(),
            ["z", "a"]
        );
        for parameter in &function.abi.parameters {
            assert_eq!(
                function
                    .semantics
                    .concrete_interface()
                    .port(&parameter.entry_output)
                    .unwrap()
                    .type_state
                    .exact(),
                Some(&parameter.value_type)
            );
        }
        assert_ne!(
            function.abi.parameters[0].entry_output,
            function.abi.parameters[1].entry_output
        );
    }

    #[test]
    fn registered_function_roles_drive_dependencies_and_abi() {
        use std::sync::Arc;
        use yss_data_contract::{SemanticType, ValueType};
        use yss_graph_document::{
            ConnectionId, DocumentConnection, LastKnownPortMetadata, OrderKey, PortAddress,
            PortInstanceId,
        };
        use yss_node_registry::{
            NodeRegistryBuilder, ProviderRegistration, RegisteredNode, StructuralNodeRole,
        };

        let function = path("Extension");
        let builtins = yss_node_catalog::build_builtin_node_system().unwrap();
        let mut builder = NodeRegistryBuilder::new();
        yss_node_catalog::register_builtin_nodes(&mut builder).unwrap();
        let mut provider = ProviderRegistration::new("tests.functions".parse().unwrap());
        provider.nodes = [
            ("call", StructuralNodeRole::Call),
            ("entry", StructuralNodeRole::FunctionEntry),
            ("return", StructuralNodeRole::FunctionReturn),
        ]
        .map(|(name, role)| {
            let mut protocol = builtins
                .registry
                .protocol(&format!("yssbi.project.function.{name}").parse().unwrap())
                .unwrap()
                .clone();
            protocol.type_id = format!("tests.function.{name}").parse().unwrap();
            let reference = &mut protocol.parameters.groups[0].parameters[0];
            reference.default_value = Some(yss_node_protocol::TypedValue {
                value_type: reference.value_type.clone(),
                value: yss_data_contract::DataValue::String(function.as_str().into()),
            });
            RegisteredNode::structural(Arc::new(protocol), role)
        })
        .into();
        let mut leaf = provider.nodes[0].protocol().clone();
        leaf.type_id = "tests.function.signature_leaf".parse().unwrap();
        let mut nodes = provider.nodes.into_vec();
        nodes.push(RegisteredNode::leaf(
            Arc::new(leaf),
            yss_node_registry::LeafImplementation::new("tests.signature"),
        ));
        provider.nodes = nodes.into();
        builder.register_provider(provider).unwrap();
        let registry = builder.freeze().unwrap();
        let numeric = ValueType::Scalar(SemanticType::Numeric);
        let signature = FunctionSignature::new(
            vec![FunctionParameterContract::new(
                FunctionParameterId::new("value"),
                "Value",
                numeric.clone(),
            )],
            Some(numeric),
        );
        let mut body = GraphDocument::default();
        node(&mut body, "tests.function.entry", "function", &function);
        node(&mut body, "tests.function.return", "function", &function);
        for node in body.nodes.values_mut() {
            node.parameters.clear();
        }
        let port = |kind: &str, template: &str| {
            let node = body
                .nodes
                .values()
                .find(|node| node.node_type.as_str() == kind)
                .unwrap();
            PortAddress::instance(node.id, template.parse().unwrap(), PortInstanceId::new())
        };
        let output = port("tests.function.entry", "parameters");
        let input = port("tests.function.return", "results");
        for (address, member) in [(&output, "value"), (&input, "return")] {
            body.port_bindings.insert(
                address.clone(),
                DynamicPortBinding::Resolved {
                    origin: DynamicMemberLocator::FunctionParameter {
                        function: function.clone(),
                        parameter: FunctionParameterId::new(member),
                    },
                    order: OrderKey::new("0"),
                    last_known: LastKnownPortMetadata::default(),
                },
            );
        }
        let id = ConnectionId::new();
        body.connections.insert(
            id,
            DocumentConnection {
                id,
                output: output.clone(),
                input: input.clone(),
                order: None,
            },
        );
        let mut root = GraphDocument::default();
        node(&mut root, "tests.function.call", "target", &function);
        root.nodes.values_mut().next().unwrap().parameters.clear();
        let resources = catalog(&[(function.clone(), signature, body)]);
        let resolution = resolve(&root, &registry, &resources);
        let resolved = resolution
            .functions
            .get(&function)
            .expect("a registered call role must capture and resolve its function");
        assert!(
            resolution.diagnostics.is_empty(),
            "{:?}",
            resolution.diagnostics
        );
        assert_eq!(resolved.abi.parameters[0].entry_output, output);
        assert_eq!(resolved.abi.result.as_ref().unwrap().return_input, input);
        root.nodes
            .values_mut()
            .next()
            .unwrap()
            .parameters
            .insert("target".parse().unwrap(), serde_json::json!(17));
        assert_eq!(direct_function_dependencies(&root, &registry).count(), 0);
        let node = root.nodes.values_mut().next().unwrap();
        node.node_type = "tests.function.signature_leaf".parse().unwrap();
        node.parameters.clear();
        let leaf = crate::resolve_graph_semantics(&root, &registry, &resources);
        assert_eq!(
            leaf.nodes()[0].ports.len(),
            2,
            "interface resolvers remain usable independently of call execution roles"
        );
        assert_eq!(direct_function_dependencies(&root, &registry).count(), 0);
    }

    #[test]
    fn restored_function_members_reuse_orphan_bindings_for_abi() {
        use yss_data_contract::{SemanticType, ValueType};
        use yss_graph_document::{
            ConnectionId, DocumentConnection, LastKnownPortMetadata, OrderKey, PortAddress,
            PortInstanceId,
        };

        let registry = yss_node_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let function = path("Restored");
        let mut body = body(&function, &[]);
        node(
            &mut body,
            "yssbi.project.function.return",
            "function",
            &function,
        );
        let port = |kind: &str, template: &str| {
            let node = body
                .nodes
                .values()
                .find(|node| node.node_type.as_str() == kind)
                .unwrap();
            PortAddress::instance(node.id, template.parse().unwrap(), PortInstanceId::new())
        };
        let output = port("yssbi.project.function.entry", "parameters");
        let input = port("yssbi.project.function.return", "results");
        for (address, member) in [(&output, "value"), (&input, "return")] {
            body.port_bindings.insert(
                address.clone(),
                DynamicPortBinding::Orphan {
                    origin: DynamicMemberLocator::FunctionParameter {
                        function: function.clone(),
                        parameter: FunctionParameterId::new(member),
                    },
                    order: OrderKey::new("0"),
                    last_known: LastKnownPortMetadata::default(),
                },
            );
        }
        let id = ConnectionId::new();
        body.connections.insert(
            id,
            DocumentConnection {
                id,
                output: output.clone(),
                input: input.clone(),
                order: None,
            },
        );
        let mut root = GraphDocument::default();
        node(
            &mut root,
            "yssbi.project.function.call",
            "target",
            &function,
        );
        let missing = catalog(&[(
            function.clone(),
            FunctionSignature::new(vec![], None),
            body.clone(),
        )]);
        assert!(
            resolve(&root, &registry, &missing)
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == "graph.function.blocked")
        );

        let numeric = ValueType::Scalar(SemanticType::Numeric);
        let restored = catalog(&[(
            function.clone(),
            FunctionSignature::new(
                vec![FunctionParameterContract::new(
                    FunctionParameterId::new("value"),
                    "Restored value",
                    numeric.clone(),
                )],
                Some(numeric),
            ),
            body.clone(),
        )]);
        let resolution = resolve(&root, &registry, &restored);
        assert!(
            resolution.diagnostics.is_empty(),
            "{:?}",
            resolution.diagnostics
        );
        let resolved = &resolution.functions[&function];
        assert!(resolved.semantics.ready().is_some());
        assert_eq!(resolved.abi.parameters[0].entry_output, output);
        assert_eq!(resolved.abi.result.as_ref().unwrap().return_input, input);
        assert_eq!(restored.function_document(&function), Some(&body));
    }

    #[test]
    fn recursion_and_entry_ownership_fail_with_canonical_function_problems() {
        let registry = yss_node_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let [a, b] = [path("A"), path("B")];
        let mut root = GraphDocument::default();
        node(&mut root, "yssbi.project.function.call", "target", &a);
        let resources = catalog(&[
            (
                a.clone(),
                FunctionSignature::new(vec![], None),
                body(&a, std::slice::from_ref(&b)),
            ),
            (
                b.clone(),
                FunctionSignature::new(vec![], None),
                body(&b, std::slice::from_ref(&a)),
            ),
        ]);
        let cycle = crate::resolve_graph_semantics(&root, &registry, &resources);
        assert!(cycle.ready().is_none());
        assert!(
            cycle
                .diagnostics()
                .iter()
                .any(
                    |diagnostic| diagnostic.code.as_str() == "graph.function.dependency_cycle"
                        && diagnostic.blocking
                )
        );
        let resources = resources.with_function_document(&b, body(&a, &[]));
        let mismatched = crate::resolve_graph_semantics(&root, &registry, &resources);
        assert!(mismatched.ready().is_none());
        assert!(
            mismatched
                .diagnostics()
                .iter()
                .any(
                    |diagnostic| diagnostic.code.as_str() == "graph.function.abi_mismatch"
                        && diagnostic.primary
                            == GraphDiagnosticLocation::Resource(b.as_str().into())
                )
        );
    }
}
