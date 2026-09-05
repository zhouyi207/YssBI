use crate::{
    GraphDiagnosticFact, GraphDiagnosticLocation, GraphPortBacking, GraphPortSemanticFact,
    GraphResolutionOutcome, GraphSemanticCache, GraphSemanticSnapshot, graph_problem,
    resolve_graph_semantics_inner,
};
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_compiler_diagnostics::GraphDiagnosticKind;
use yss_graph_document::{
    DynamicMemberLocator, DynamicPortBinding, FunctionParameterId, GraphDocument,
    GraphResourcePath, PortAddress,
};
use yss_graph_protocol::{PortDirection, ResolvedType};
use yss_graph_registry::NodeRegistry;
use yss_graph_resource_contract::{FunctionSignature, ResourceCatalogSnapshot};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionParameter {
    pub id: FunctionParameterId,
    pub entry_output: PortAddress,
    pub value_type: ResolvedType,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionResult {
    pub id: FunctionParameterId,
    pub return_input: PortAddress,
    pub value_type: ResolvedType,
}

/// Parameter array order is the signature order; labels never identify ABI slots.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionAbi {
    pub parameters: Box<[GraphFunctionParameter]>,
    pub result: Option<GraphFunctionResult>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphFunctionSemanticFact {
    pub abi: GraphFunctionAbi,
    pub semantics: GraphSemanticSnapshot,
}

#[derive(Default)]
pub(crate) struct FunctionResolution {
    pub diagnostics: Vec<GraphDiagnosticFact>,
    pub functions: BTreeMap<GraphResourcePath, GraphFunctionSemanticFact>,
    pub internal_failure: Option<GraphResolutionOutcome>,
}

fn callees(document: &GraphDocument) -> Vec<GraphResourcePath> {
    document
        .nodes
        .values()
        .filter(|node| node.node_type.as_str() == "yssbi.project.function.call")
        .filter_map(|node| {
            node.parameters
                .iter()
                .find(|(key, _)| key.as_str() == "target")
                .and_then(|(_, value)| value.as_str())
                .and_then(|path| GraphResourcePath::new(path).ok())
        })
        .collect()
}

pub(crate) fn resolve(
    document: &GraphDocument,
    registry: &NodeRegistry,
    resources: &ResourceCatalogSnapshot,
) -> FunctionResolution {
    let mut pending = callees(document)
        .into_iter()
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
            .filter(|node| node.node_type.as_str() == "yssbi.project.function.entry")
            .collect::<Vec<_>>();
        let returns = body
            .nodes
            .values()
            .filter(|node| node.node_type.as_str() == "yssbi.project.function.return")
            .collect::<Vec<_>>();
        let mismatched_owner = entries.iter().chain(returns.iter()).any(|node| {
            node.parameters
                .iter()
                .find(|(key, _)| key.as_str() == "function")
                .and_then(|(_, value)| value.as_str())
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
            false,
        );
        if matches!(
            semantics.outcome(),
            GraphResolutionOutcome::InternalFailure { .. }
        ) {
            // A resolver failure inside a callee remains an internal failure at the root.
            resolution.internal_failure = Some(GraphResolutionOutcome::InternalFailure {
                stage: crate::GraphCompilationStage::Analysis,
                code: "compiler.function.resolution_failed".into(),
                node_id: None,
            });
        } else if semantics.ready().is_none() {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionBlocked, &path));
        }
        if let Some(abi) = resolve_abi(&path, body, signature, &semantics) {
            resolution
                .functions
                .insert(path.clone(), GraphFunctionSemanticFact { abi, semantics });
        } else {
            diagnostics.push(problem(GraphDiagnosticKind::FunctionAbiMismatch, &path));
        }
        active.insert(path.clone());
        pending.push((path, true));
        pending.extend(callees(body).into_iter().map(|callee| (callee, false)));
    }
    resolution
}

fn resolve_abi(
    path: &GraphResourcePath,
    document: &GraphDocument,
    signature: &FunctionSignature,
    semantics: &GraphSemanticSnapshot,
) -> Option<GraphFunctionAbi> {
    let entry = semantics
        .nodes()
        .iter()
        .find(|node| node.node_type.as_str() == "yssbi.project.function.entry")?;
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
            let return_node = semantics
                .nodes()
                .iter()
                .find(|node| node.node_type.as_str() == "yssbi.project.function.return")?;
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
            _ => document.port_bindings.get(&port.address).and_then(|binding| match binding {
                DynamicPortBinding::Resolved { origin, .. } => Some(origin),
                _ => None,
            }),
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
    use yss_graph_document::{DocumentNode, NodeId, NodePosition, ParameterValues};
    use yss_graph_resource_contract::{
        FunctionCatalogEntry, FunctionParameterContract, ResourceCatalogFingerprint,
    };

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
            BTreeMap::new(),
            ResourceCatalogFingerprint::from_bytes([0; 32]),
        );
        functions.iter().fold(catalog, |catalog, (path, _, body)| {
            catalog.with_function_document(path, body.clone())
        })
    }

    #[test]
    fn reachable_functions_share_one_snapshot_and_abi_uses_signature_identity_order() {
        let registry = yss_graph_catalog::build_builtin_node_system()
            .unwrap()
            .registry;
        let [a, b, c] = [path("A"), path("B"), path("C")];
        let signature = FunctionSignature::new(
            vec![
                FunctionParameterContract::new(
                    FunctionParameterId::new("z"),
                    "Value",
                    yss_data_contract::DataType::Int64,
                ),
                FunctionParameterContract::new(
                    FunctionParameterId::new("a"),
                    "Value",
                    yss_data_contract::DataType::Float64,
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
    fn recursion_and_entry_ownership_fail_with_canonical_function_problems() {
        let registry = yss_graph_catalog::build_builtin_node_system()
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
                    |diagnostic| diagnostic.code.as_str() == "compiler.function.dependency_cycle"
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
                    |diagnostic| diagnostic.code.as_str() == "compiler.function.abi_mismatch"
                        && diagnostic.primary
                            == GraphDiagnosticLocation::Resource(b.as_str().into())
                )
        );
    }
}
