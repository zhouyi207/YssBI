use std::collections::BTreeMap;

use crate::graph::analysis::{GraphAnalysis, GraphAnalysisInput, analyze};
use crate::graph::compiler::package::{
    GraphCompiledPackage, GraphInputBinding, GraphInputSource, GraphObservationIntent,
    GraphOperation, GraphParameterHandle, GraphParameterPayload, GraphParameterScalar,
    GraphParameterValue, GraphSourceIdentity, GraphValueRef,
};
use crate::graph::error::GraphCompileError;
use crate::graph::settings::GraphCompileSettings;
use yss_graph_analysis_contract::{CompilationBasis, CompileId};
use yss_graph_document::{GraphDocument, GraphResourcePath, GraphRevision};
use yss_graph_resource_contract::ResourceCatalogSnapshot;

const DEBUG_VIEW_NODE_TYPE: &str = "yssbi.debug.view";

pub(crate) struct GraphCompilationInput<'a> {
    document: &'a GraphDocument,
    catalog: &'a ResourceCatalogSnapshot,
    settings: &'a GraphCompileSettings,
    basis: CompilationBasis<GraphRevision>,
    graph: GraphResourcePath,
    compile_id: CompileId,
}

impl<'a> GraphCompilationInput<'a> {
    pub(crate) fn new(
        document: &'a GraphDocument,
        catalog: &'a ResourceCatalogSnapshot,
        settings: &'a GraphCompileSettings,
        basis: CompilationBasis<GraphRevision>,
        graph: GraphResourcePath,
        compile_id: CompileId,
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
pub(crate) struct GraphDiagnostic {
    pub(crate) code: Box<str>,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct CompilationReport {
    pub(crate) analysis: GraphAnalysis,
    pub(crate) diagnostics: Box<[GraphDiagnostic]>,
    pub(crate) executable: Option<GraphCompiledPackage>,
    pub(crate) basis: CompilationBasis<GraphRevision>,
}

pub(crate) fn compile(
    input: GraphCompilationInput<'_>,
) -> Result<CompilationReport, GraphCompileError> {
    if input.basis.graph_revision != input.document.revision {
        return Err(GraphCompileError::InvalidGraph {
            graph: input.graph,
            code: crate::graph::error::GraphCompileErrorCode::InvalidDocument,
        });
    }

    let analysis = analyze(GraphAnalysisInput {
        document: input.document,
        catalog: input.catalog,
        settings: input.settings,
        basis: &input.basis,
    });
    let package = lower_package(
        input.document,
        input.graph.clone(),
        input.compile_id,
        &input.graph,
        input.basis.clone(),
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
    graph: GraphResourcePath,
    compile_id: CompileId,
    graph_path: &GraphResourcePath,
    basis: CompilationBasis<GraphRevision>,
) -> Result<GraphCompiledPackage, GraphCompileError> {
    let value_refs = node_value_refs(document, graph_path)?;
    let operations = document
        .nodes
        .values()
        .map(|node| {
            let value_ref = value_refs
                .get(&node.id)
                .copied()
                .ok_or_else(|| lowering_error(graph_path))?;
            let result_category = crate::graph::analysis::result_category::result_category_for_node(
                node.node_type.as_str(),
            );
            let parameter_handles = node
                .parameters
                .keys()
                .map(|key| node_parameter_handle(node.id, key.as_str()))
                .collect::<Vec<_>>()
                .into_boxed_slice();
            let inputs = lower_input_bindings(node.id, document, &value_refs, graph_path)?;
            let observation_intents: Box<[GraphObservationIntent]> = if node.node_type.as_str()
                == DEBUG_VIEW_NODE_TYPE
            {
                vec![GraphObservationIntent::InspectInput { input: value_ref }].into_boxed_slice()
            } else {
                Box::new([])
            };
            Ok(GraphOperation::new(
                GraphSourceIdentity::new(graph.clone(), Some(node.id), None),
                node.node_type.as_str(),
                result_category,
                parameter_handles,
                inputs,
                observation_intents,
                Some(value_ref),
            ))
        })
        .collect::<Result<Vec<_>, GraphCompileError>>()?;

    let parameters = lower_parameters(document, graph_path)?;
    Ok(GraphCompiledPackage::new(
        basis,
        compile_id,
        operations.into_boxed_slice(),
        parameters,
    ))
}

fn lower_parameters(
    document: &GraphDocument,
    graph_path: &GraphResourcePath,
) -> Result<BTreeMap<GraphParameterHandle, GraphParameterPayload>, GraphCompileError> {
    let mut parameters = BTreeMap::new();
    for node in document.nodes.values() {
        for (key, value) in &node.parameters {
            let handle = node_parameter_handle(node.id, key.as_str());
            let schema = format!("node/{}/{}", node.node_type.as_str(), key.as_str());
            let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
            parameters.insert(handle, GraphParameterPayload::new(schema, value));
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
        let handle = input_parameter_handle(&port);
        let schema = format!("input/{port}");
        let value = lower_parameter_value(value).map_err(|_| lowering_error(graph_path))?;
        parameters.insert(handle, GraphParameterPayload::new(schema, value));
    }
    Ok(parameters)
}

fn node_value_refs(
    document: &GraphDocument,
    graph_path: &GraphResourcePath,
) -> Result<BTreeMap<yss_graph_document::NodeId, GraphValueRef>, GraphCompileError> {
    document
        .nodes
        .keys()
        .copied()
        .enumerate()
        .map(|(index, node_id)| {
            u32::try_from(index)
                .map(|index| (node_id, GraphValueRef::new(index)))
                .map_err(|_| lowering_error(graph_path))
        })
        .collect()
}

fn lower_input_bindings(
    node_id: yss_graph_document::NodeId,
    document: &GraphDocument,
    value_refs: &BTreeMap<yss_graph_document::NodeId, GraphValueRef>,
    graph_path: &GraphResourcePath,
) -> Result<Box<[GraphInputBinding]>, GraphCompileError> {
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
        bindings.push(GraphInputBinding::new(
            connection.input.to_string(),
            GraphInputSource::Value(source),
        ));
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
        bindings.push(GraphInputBinding::new(
            port_text.clone(),
            GraphInputSource::Parameter(input_parameter_handle(&port_text)),
        ));
    }
    Ok(bindings.into_boxed_slice())
}

fn node_parameter_handle(
    node_id: yss_graph_document::NodeId,
    parameter: &str,
) -> GraphParameterHandle {
    GraphParameterHandle::new(format!("node/{node_id}/{parameter}"))
}

fn input_parameter_handle(port: &str) -> GraphParameterHandle {
    GraphParameterHandle::new(format!("input/{port}"))
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
enum ParameterLoweringError {
    NonFiniteDecimal,
    UnsupportedValue,
}

fn lower_parameter_value(
    value: &yss_graph_document::TypedValue,
) -> Result<GraphParameterValue, ParameterLoweringError> {
    if value.is_null() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Null));
    }
    if let Some(value) = value.as_bool() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Bool(
            value,
        )));
    }
    if let Some(value) = value.as_i64() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Integer(
            value,
        )));
    }
    if let Some(value) = value.as_u64() {
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Unsigned(
            value,
        )));
    }
    if let Some(value) = value.as_f64() {
        if !value.is_finite() {
            return Err(ParameterLoweringError::NonFiniteDecimal);
        }
        return Ok(GraphParameterValue::Scalar(GraphParameterScalar::Decimal(
            value,
        )));
    }
    if let Some(value) = value.as_str() {
        return Ok(
            if ["events/", "functions/", "variables/", "databases/"]
                .into_iter()
                .any(|prefix| value.starts_with(prefix))
            {
                GraphParameterValue::Resource(value.to_owned().into_boxed_str())
            } else {
                GraphParameterValue::Scalar(GraphParameterScalar::String(
                    value.to_owned().into_boxed_str(),
                ))
            },
        );
    }
    if let Some(values) = value.as_array() {
        return values
            .iter()
            .map(lower_parameter_value)
            .collect::<Result<Vec<_>, _>>()
            .map(|values| GraphParameterValue::List(values.into_boxed_slice()));
    }
    if let Some(values) = value.as_object() {
        let fields = values
            .iter()
            .map(|(field, value)| {
                Ok((
                    field.clone().into_boxed_str(),
                    lower_parameter_value(value)?,
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ParameterLoweringError>>()?;
        return Ok(GraphParameterValue::Record(fields));
    }
    Err(ParameterLoweringError::UnsupportedValue)
}

fn lowering_error(graph: &GraphResourcePath) -> GraphCompileError {
    GraphCompileError::InvalidGraph {
        graph: graph.clone(),
        code: crate::graph::error::GraphCompileErrorCode::LoweringInvariant,
    }
}
