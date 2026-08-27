use std::collections::BTreeMap;
use std::sync::Arc;

use crate::execution::plan::{
    CanonicalDecimal, CompiledExecutionPackage, CompiledFunctionBundle,
    CompiledParameterBundleBuilder, CompiledParameterHandle, ExecutionPlan, PlanCompilationBasis,
    PlanCompileId, PlanGraphId, PlanInputBinding, PlanInputSource, PlanNodeId,
    PlanObservationIntent, PlanOperation, PlanOperationKind, PlanParameterFieldId,
    PlanParameterPayload, PlanParameterScalar, PlanParameterSchemaId, PlanParameterValue,
    PlanProvenance, PlanResourceId, PlanSourceIdentity, ValueRef,
};
use crate::graph::analysis::{GraphAnalysis, GraphAnalysisInput, analyze};
use crate::graph::error::GraphCompileError;
use crate::graph::resource_catalog::ResourceCatalogSnapshot;
use crate::graph::settings::GraphCompileSettings;
use crate::graph_document::{GraphDocument, GraphResourcePath};

const DEBUG_VIEW_NODE_TYPE: &str = "yssbi.debug.view";

pub struct GraphCompilationInput<'a> {
    document: &'a GraphDocument,
    catalog: &'a ResourceCatalogSnapshot,
    settings: &'a GraphCompileSettings,
    basis: PlanCompilationBasis,
    graph: GraphResourcePath,
    compile_id: PlanCompileId,
}

impl<'a> GraphCompilationInput<'a> {
    pub fn new(
        document: &'a GraphDocument,
        catalog: &'a ResourceCatalogSnapshot,
        settings: &'a GraphCompileSettings,
        basis: PlanCompilationBasis,
        graph: GraphResourcePath,
        compile_id: PlanCompileId,
    ) -> Self {
        Self {
            document,
            catalog,
            settings,
            basis,
            graph,
            compile_id,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GraphDiagnostic {
    pub code: Box<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct CompilationReport {
    pub analysis: GraphAnalysis,
    pub diagnostics: Box<[GraphDiagnostic]>,
    pub executable: Option<CompiledExecutionPackage>,
    pub basis: PlanCompilationBasis,
}

pub fn compile(input: GraphCompilationInput<'_>) -> Result<CompilationReport, GraphCompileError> {
    if input.basis.graph_revision().get() != input.document.revision.get() {
        return Err(GraphCompileError::InvalidGraph {
            graph: input.graph,
            code: crate::graph::error::GraphCompileErrorCode::InvalidDocument,
        });
    }

    let plan_graph =
        PlanGraphId::new(input.graph.as_str().to_owned().into_boxed_str()).map_err(|_| {
            GraphCompileError::InvalidGraph {
                graph: input.graph.clone(),
                code: crate::graph::error::GraphCompileErrorCode::LoweringInvariant,
            }
        })?;
    let analysis = analyze(GraphAnalysisInput {
        document: input.document,
        catalog: input.catalog,
        settings: input.settings,
        basis: &input.basis,
    });
    let package = lower_package(
        input.document,
        &input.basis,
        plan_graph,
        input.compile_id,
        &input.graph,
    )?;
    Ok(CompilationReport {
        analysis,
        diagnostics: Box::new([]),
        executable: Some(package),
        basis: input.basis,
    })
}

fn lower_package(
    document: &GraphDocument,
    basis: &PlanCompilationBasis,
    graph: PlanGraphId,
    compile_id: PlanCompileId,
    graph_path: &GraphResourcePath,
) -> Result<CompiledExecutionPackage, GraphCompileError> {
    let value_refs = node_value_refs(document, graph_path)?;
    let operations = document
        .nodes
        .values()
        .enumerate()
        .map(|(_index, node)| {
            let value_ref = value_refs
                .get(&node.id)
                .copied()
                .ok_or_else(|| lowering_error(graph_path))?;
            let node_id = PlanNodeId::new(node.id.to_string().into_boxed_str())
                .map_err(|_| lowering_error(graph_path))?;
            let kind = PlanOperationKind::new(node.node_type.as_str().to_owned().into_boxed_str())
                .map_err(|_| lowering_error(graph_path))?;
            let parameter_handles = node
                .parameters
                .keys()
                .map(|key| node_parameter_handle(node.id, key.as_str(), graph_path))
                .collect::<Result<Vec<_>, _>>()?
                .into_boxed_slice();
            let inputs = lower_input_bindings(node.id, document, &value_refs, graph_path)?;
            let observation_intents: Box<[PlanObservationIntent]> = if node.node_type.as_str()
                == DEBUG_VIEW_NODE_TYPE
            {
                vec![PlanObservationIntent::InspectInput { input: value_ref }].into_boxed_slice()
            } else {
                Box::new([])
            };
            Ok(PlanOperation::new(
                PlanSourceIdentity::new(graph.clone(), Some(node_id), None),
                kind,
                parameter_handles,
                inputs,
                observation_intents,
                Some(value_ref),
            ))
        })
        .collect::<Result<Vec<_>, GraphCompileError>>()?;

    let parameters = lower_parameters(document, basis, graph_path)?;
    let plan = Arc::new(ExecutionPlan::new(operations.into_boxed_slice()));
    let functions = Arc::new(CompiledFunctionBundle::new(basis.clone(), Box::new([]), 0));
    let package = CompiledExecutionPackage::new(
        plan,
        functions,
        Arc::new(parameters),
        PlanProvenance::new(
            PlanSourceIdentity::new(graph, None, None),
            basis.clone(),
            compile_id,
        ),
    );
    package.validate().map_err(|_| lowering_error(graph_path))?;
    Ok(package)
}

fn lower_parameters(
    document: &GraphDocument,
    basis: &PlanCompilationBasis,
    graph_path: &GraphResourcePath,
) -> Result<crate::execution::plan::CompiledParameterBundle, GraphCompileError> {
    let mut builder = CompiledParameterBundleBuilder::new(basis.clone());
    for node in document.nodes.values() {
        for (key, value) in &node.parameters {
            let handle = CompiledParameterHandle::new(
                format!("node/{}/{}", node.id, key.as_str()).into_boxed_str(),
            )
            .map_err(|_| lowering_error(graph_path))?;
            let schema = PlanParameterSchemaId::new(
                format!("node/{}/{}", node.node_type.as_str(), key.as_str()).into_boxed_str(),
            )
            .map_err(|_| lowering_error(graph_path))?;
            let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
            builder
                .insert(handle, PlanParameterPayload::new(schema, value))
                .map_err(|_| lowering_error(graph_path))?;
        }
    }
    for (port, state) in &document.input_states {
        let Some(value) = state.literal_override.as_ref() else {
            continue;
        };
        if document
            .connections
            .values()
            .any(|connection| connection.input == *port)
        {
            continue;
        }
        let port = port.to_string();
        let handle = input_parameter_handle(&port, graph_path)?;
        let schema = PlanParameterSchemaId::new(format!("input/{port}").into_boxed_str())
            .map_err(|_| lowering_error(graph_path))?;
        let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
        builder
            .insert(handle, PlanParameterPayload::new(schema, value))
            .map_err(|_| lowering_error(graph_path))?;
    }
    Ok(builder.freeze())
}

fn node_value_refs(
    document: &GraphDocument,
    graph_path: &GraphResourcePath,
) -> Result<BTreeMap<crate::graph_document::NodeId, ValueRef>, GraphCompileError> {
    document
        .nodes
        .keys()
        .copied()
        .enumerate()
        .map(|(index, node_id)| {
            u32::try_from(index)
                .map(|index| (node_id, ValueRef::new(index)))
                .map_err(|_| lowering_error(graph_path))
        })
        .collect()
}

fn lower_input_bindings(
    node_id: crate::graph_document::NodeId,
    document: &GraphDocument,
    value_refs: &BTreeMap<crate::graph_document::NodeId, ValueRef>,
    graph_path: &GraphResourcePath,
) -> Result<Box<[PlanInputBinding]>, GraphCompileError> {
    let mut bindings = Vec::new();
    for connection in document
        .connections
        .values()
        .filter(|connection| connection.input.node_id == node_id)
    {
        let source = value_refs
            .get(&connection.output.node_id)
            .copied()
            .ok_or_else(|| lowering_error(graph_path))?;
        let port = crate::execution::plan::PlanPortAddress::new(
            connection.input.to_string().into_boxed_str(),
        )
        .map_err(|_| lowering_error(graph_path))?;
        bindings.push(PlanInputBinding::new(port, PlanInputSource::Value(source)));
    }
    for (port, state) in &document.input_states {
        if port.node_id != node_id || state.literal_override.is_none() {
            continue;
        }
        if document
            .connections
            .values()
            .any(|connection| connection.input == *port)
        {
            continue;
        }
        let port_text = port.to_string();
        let plan_port =
            crate::execution::plan::PlanPortAddress::new(port_text.clone().into_boxed_str())
                .map_err(|_| lowering_error(graph_path))?;
        let handle = input_parameter_handle(&port_text, graph_path)?;
        bindings.push(PlanInputBinding::new(
            plan_port,
            PlanInputSource::Parameter(handle),
        ));
    }
    Ok(bindings.into_boxed_slice())
}

fn node_parameter_handle(
    node_id: crate::graph_document::NodeId,
    parameter: &str,
    graph_path: &GraphResourcePath,
) -> Result<CompiledParameterHandle, GraphCompileError> {
    CompiledParameterHandle::new(format!("node/{node_id}/{parameter}").into_boxed_str())
        .map_err(|_| lowering_error(graph_path))
}

fn input_parameter_handle(
    port: &str,
    graph_path: &GraphResourcePath,
) -> Result<CompiledParameterHandle, GraphCompileError> {
    CompiledParameterHandle::new(format!("input/{port}").into_boxed_str())
        .map_err(|_| lowering_error(graph_path))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ParameterLoweringError {
    InvalidIdentity,
    NonFiniteDecimal,
    UnsupportedValue,
}

fn lower_parameter_value(
    value: &crate::graph_document::TypedValue,
) -> Result<PlanParameterValue, ParameterLoweringError> {
    if value.is_null() {
        return Ok(PlanParameterValue::Scalar(PlanParameterScalar::Null));
    }
    if let Some(value) = value.as_bool() {
        return Ok(PlanParameterValue::Scalar(PlanParameterScalar::Bool(value)));
    }
    if let Some(value) = value.as_i64() {
        return Ok(PlanParameterValue::Scalar(PlanParameterScalar::Integer(
            value,
        )));
    }
    if let Some(value) = value.as_u64() {
        return Ok(PlanParameterValue::Scalar(PlanParameterScalar::Unsigned(
            value,
        )));
    }
    if let Some(value) = value.as_f64() {
        let value = CanonicalDecimal::try_new(value)
            .map_err(|_| ParameterLoweringError::NonFiniteDecimal)?;
        return Ok(PlanParameterValue::Scalar(PlanParameterScalar::Decimal(
            value,
        )));
    }
    if let Some(value) = value.as_str() {
        return lower_string_value(value);
    }
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(lower_parameter_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| PlanParameterValue::List(values.into_boxed_slice()));
    }
    if let Some(values) = value.as_object() {
        let mut fields = BTreeMap::new();
        for (field, value) in values {
            let field = PlanParameterFieldId::new(field.clone().into_boxed_str())
                .map_err(|_| ParameterLoweringError::InvalidIdentity)?;
            let value = lower_parameter_value(value)?;
            fields.insert(field, value);
        }
        return Ok(PlanParameterValue::Record(fields));
    }
    Err(ParameterLoweringError::UnsupportedValue)
}

fn lower_string_value(value: &str) -> Result<PlanParameterValue, ParameterLoweringError> {
    let is_resource = ["events/", "functions/", "variables/", "databases/"]
        .into_iter()
        .any(|prefix| value.starts_with(prefix));
    if is_resource {
        PlanResourceId::new(value.to_owned().into_boxed_str())
            .map(PlanParameterValue::Resource)
            .map_err(|_| ParameterLoweringError::InvalidIdentity)
    } else {
        Ok(PlanParameterValue::Scalar(PlanParameterScalar::String(
            value.to_owned().into_boxed_str(),
        )))
    }
}

fn lowering_error(graph: &GraphResourcePath) -> GraphCompileError {
    GraphCompileError::InvalidGraph {
        graph: graph.clone(),
        code: crate::graph::error::GraphCompileErrorCode::LoweringInvariant,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::execution::plan::{
        PlanCompileId, PlanGraphRevision, PlanParameterFieldId, PlanParameterValue,
        PlanProjectSessionId, PlanRegistryFingerprint, ValueRef,
    };
    use crate::graph::resource_catalog::ResourceCatalogSnapshot;
    use crate::graph_document::{DocumentNode, GraphResourcePath, NodeId, NodePosition};
    use crate::node_system::protocol::{NodeTypeId, ParameterKey};
    use serde_json::json;
    use std::collections::BTreeMap;

    fn basis(revision: u64) -> PlanCompilationBasis {
        PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("project".into()),
            PlanGraphRevision::from_existing(revision),
            PlanRegistryFingerprint::from_bytes([2; 32]),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    fn catalog() -> ResourceCatalogSnapshot {
        ResourceCatalogSnapshot::new(
            BTreeMap::new(),
            BTreeMap::new(),
            BTreeMap::new(),
            crate::graph::resource_catalog::ResourceCatalogFingerprint::from_bytes([1; 32]),
        )
    }

    #[test]
    fn compiler_lowers_parameters_and_debug_observation_into_neutral_package() {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(1));
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.debug.view").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::from([(
                    ParameterKey::new("options").unwrap(),
                    json!({
                        "enabled": true,
                        "limit": 7,
                        "ratio": 1.25,
                        "labels": ["first", null]
                    }),
                )]),
                user_label: None,
            },
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let graph = GraphResourcePath::new("events/main.yssbi-event").unwrap();
        let report = compile(GraphCompilationInput::new(
            &document,
            &catalog(),
            &settings,
            basis(document.revision.get()),
            graph,
            PlanCompileId::from_existing(7),
        ))
        .unwrap();

        let package = report
            .executable
            .expect("valid graph has a neutral package");
        assert_eq!(package.plan().operations().len(), 1);
        assert_eq!(package.provenance().compile_id().get(), 7);
        assert_eq!(package.provenance().source().node(), None);
        assert_eq!(
            package.plan().operations()[0].observation_intents(),
            &[
                crate::execution::plan::PlanObservationIntent::InspectInput {
                    input: ValueRef::new(0),
                }
            ]
        );

        let payload = package.parameters().entries().values().next().unwrap();
        let PlanParameterValue::Record(fields) = payload.value() else {
            panic!("expected a closed neutral record");
        };
        assert!(matches!(
            fields.get(&PlanParameterFieldId::from_existing("enabled".into())),
            Some(PlanParameterValue::Scalar(
                crate::execution::plan::PlanParameterScalar::Bool(true)
            ))
        ));
        assert!(matches!(
            fields.get(&PlanParameterFieldId::from_existing("labels".into())),
            Some(PlanParameterValue::List(values)) if values.len() == 2
        ));
    }

    #[test]
    fn compiler_rejects_noncanonical_parameter_field_id() {
        let node_id = NodeId::from_uuid(uuid::Uuid::from_u128(2));
        let mut document = GraphDocument::default();
        document.nodes.insert(
            node_id,
            DocumentNode {
                id: node_id,
                node_type: NodeTypeId::new("yssbi.test.node").unwrap(),
                position: NodePosition { x: 0.0, y: 0.0 },
                parameters: BTreeMap::from([(
                    ParameterKey::new("options").unwrap(),
                    json!({" ": true}),
                )]),
                user_label: None,
            },
        );
        let settings = GraphCompileSettings {
            absolute_tolerance: 1e-12,
            relative_tolerance: 1e-9,
        };
        let graph = GraphResourcePath::new("events/main.yssbi-event").unwrap();
        let error = compile(GraphCompilationInput::new(
            &document,
            &catalog(),
            &settings,
            basis(document.revision.get()),
            graph,
            PlanCompileId::from_existing(8),
        ))
        .expect_err("invalid plan field identity must fail closed");

        assert!(matches!(
            error,
            GraphCompileError::InvalidGraph {
                code: crate::graph::error::GraphCompileErrorCode::LoweringInvariant,
                ..
            }
        ));
    }
}
