//! Build execution plans from the editor's resolved graph facts.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use yss_graph_analysis::{
    GraphAnalysis, GraphNodeSemanticFact, GraphPlotDataKind, GraphPortSemanticFact,
    GraphResolvedInputSource, GraphResolvedParameterValue, GraphResultCategory,
    GraphSemanticSnapshot, GraphStatisticalReportKind,
};
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_node_kernel::KernelId;
use yss_node_kernel::KernelParameterKey;
use yss_node_protocol::{PortDirection, Value};

use crate::plan::*;
use crate::state::ExecutionRuntimeState;

#[derive(Debug, Error)]
pub enum GraphPlanError {
    #[error("graph is not ready for execution")]
    NotReady,
    #[error("graph execution capabilities do not match the active runtime")]
    KernelCapabilitiesMismatch,
    #[error("resolved graph plan contains inconsistent references")]
    InvalidGraph,
    #[error("graph parameter literal is invalid")]
    InvalidLiteral,
    #[error("graph plan identity is invalid")]
    Identity(#[from] InvalidPlanIdentity),
    #[error("graph parameter identity is invalid")]
    ParameterIdentity(#[from] InvalidPlanParameterId),
    #[error("kernel identity is invalid")]
    KernelIdentity(#[from] yss_node_kernel::InvalidKernelIdentity),
    #[error("graph parameter is not a finite decimal")]
    Decimal(#[from] CanonicalDecimalError),
    #[error("graph parameter handle is duplicated")]
    DuplicateParameter(#[from] PlanParameterBundleError),
    #[error("resolved graph type is unsupported by execution")]
    UnsupportedResolvedType,
    #[error("graph constant cannot be represented at runtime")]
    ConstantValue(#[from] yss_node_kernel::RuntimeValueError),
}

#[derive(Debug)]
struct GraphPlanTemplate {
    graph: PlanGraphId,
    plan_id: PlanId,
    plan: Arc<ExecutionPlan>,
    parameters: BTreeMap<PlanParameterHandle, PlanParameterPayload>,
}

struct CachedGraphPlan {
    semantic_input_hash: [u8; 32],
    template: Arc<GraphPlanTemplate>,
}

#[derive(Default)]
pub(crate) struct GraphPlanCache(Mutex<BTreeMap<String, CachedGraphPlan>>);

impl GraphPlanCache {
    pub(crate) fn remove(&self, graph: &str) {
        self.0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .remove(graph);
    }
}

impl ExecutionRuntimeState {
    /// Plan reuse is internal to the execution session; readiness is rechecked on every request.
    pub fn prepare_graph_package(
        &self,
        graph: &GraphResourcePath,
        analysis: &GraphAnalysis,
        basis: PlanBasis,
    ) -> Result<ExecutionPlanPackage, GraphPlanError> {
        if analysis.kernel_fingerprint() != &basis.kernel_fingerprint().as_bytes()
            || basis.kernel_fingerprint() != self.kernels().fingerprint()
        {
            return Err(GraphPlanError::KernelCapabilitiesMismatch);
        }
        let semantics = analysis.semantic_snapshot();
        semantics.ready().ok_or(GraphPlanError::NotReady)?;
        let hash = *analysis.semantic_input_hash();
        let cached = self
            .graph_plans
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(graph.as_str())
            .filter(|cached| cached.semantic_input_hash == hash)
            .map(|cached| Arc::clone(&cached.template));
        let template = if let Some(template) = cached {
            template
        } else {
            let plan_id = PlanId::from_existing(u64::from_be_bytes(
                hash[..8].try_into().expect("SHA-256 prefix"),
            ));
            let template = Arc::new(build_template(graph, plan_id, semantics)?);
            self.graph_plans
                .0
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .insert(
                    graph.as_str().into(),
                    CachedGraphPlan {
                        semantic_input_hash: hash,
                        template: Arc::clone(&template),
                    },
                );
            template
        };
        let mut parameters = PlanParameterBundleBuilder::new(basis.clone());
        for (handle, payload) in &template.parameters {
            parameters.insert(handle.clone(), payload.clone())?;
        }
        Ok(ExecutionPlanPackage::new(
            Arc::clone(&template.plan),
            Arc::new(parameters.freeze()),
            PlanProvenance::new(
                PlanSourceIdentity::new(template.graph.clone(), None, None),
                basis,
                template.plan_id,
            ),
        ))
    }
}

fn build_template(
    graph: &GraphResourcePath,
    plan_id: PlanId,
    semantics: &GraphSemanticSnapshot,
) -> Result<GraphPlanTemplate, GraphPlanError> {
    semantics.ready().ok_or(GraphPlanError::NotReady)?;
    let graph = PlanGraphId::new(graph.as_str().into())?;
    let outputs = semantics
        .nodes()
        .iter()
        .flat_map(|node| node.ports.iter())
        .filter(|port| port.direction == PortDirection::Output && !port.orphan)
        .enumerate()
        .map(|(index, port)| {
            Ok((
                port.address.clone(),
                ValueRef::new(u32::try_from(index).map_err(|_| GraphPlanError::InvalidGraph)?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, GraphPlanError>>()?;
    let mut parameters = BTreeMap::new();
    let mut operations = Vec::new();
    for node in semantics.nodes() {
        let mut handles = BTreeMap::new();
        if let Some(constant) = &node.constant {
            let handle = parameter_handle(format!("constant/{}", constant.id));
            parameters.insert(
                handle.clone(),
                PlanParameterPayload::new(
                    parameter_schema("graph.constant".into()),
                    PlanParameterValue::Literal(Arc::new(constant_runtime_value(constant)?)),
                ),
            );
            handles.insert(KernelParameterKey::from_existing("value".into()), handle);
        } else {
            for parameter in &node.parameters {
                let Some(value) = &parameter.effective_value else {
                    continue;
                };
                let handle = parameter_handle(format!("node/{}/{}", node.node_id, parameter.key));
                let value = match value {
                    GraphResolvedParameterValue::Resource(resource) => {
                        PlanParameterValue::Resource(PlanResourceId::new(resource.as_str().into())?)
                    }
                    GraphResolvedParameterValue::Literal(value) => parameter_value(value)?,
                    GraphResolvedParameterValue::DefaultLiteral(value) => protocol_value(value)?,
                };
                parameters.insert(
                    handle.clone(),
                    PlanParameterPayload::new(
                        parameter_schema(format!("node/{}/{}", node.node_type, parameter.key)),
                        value,
                    ),
                );
                handles.insert(
                    KernelParameterKey::new(parameter.key.as_str().into())?,
                    handle,
                );
            }
        }
        let inputs = input_bindings(node, &outputs, &mut parameters)?;
        let observations: Box<[PlanObservationIntent]> =
            if node.node_type.as_str() == "yssbi.debug.view" {
                inputs
                    .first()
                    .map(|input| PlanObservationIntent::InspectInput {
                        source: input.source().clone(),
                    })
                    .into_iter()
                    .collect()
            } else {
                Box::new([])
            };
        let bindings = node
            .ports
            .iter()
            .filter(|port| port.direction == PortDirection::Output && !port.orphan)
            .map(|port| {
                Ok(PlanOutputBinding::new(
                    PlanOutputRef::new(graph.clone(), plan_port(&port.address)),
                    *outputs
                        .get(&port.address)
                        .ok_or(GraphPlanError::InvalidGraph)?,
                    output_contract(port, &graph)?,
                ))
            })
            .collect::<Result<Box<[_]>, GraphPlanError>>()?;
        operations.push(PlanOperation::new(
            PlanSourceIdentity::new(
                graph.clone(),
                Some(PlanNodeId::from_existing(node.node_id.to_string().into())),
                None,
            ),
            PlanNodeTypeId::new(node.node_type.as_str().into())?,
            handles,
            inputs,
            observations,
            bindings,
            plan_specialization(
                node.specialization
                    .as_ref()
                    .ok_or(GraphPlanError::NotReady)?,
            )?,
        ));
    }
    Ok(GraphPlanTemplate {
        graph,
        plan_id,
        plan: Arc::new(ExecutionPlan::new(operations.into_boxed_slice())),
        parameters,
    })
}

fn plan_port(port: &PortAddress) -> PlanPortAddress {
    PlanPortAddress::from_existing(port.to_string().into())
}

fn parameter_handle(value: String) -> PlanParameterHandle {
    PlanParameterHandle::from_existing(value.into())
}

fn parameter_schema(value: String) -> PlanParameterSchemaId {
    PlanParameterSchemaId::from_existing(value.into())
}

fn input_bindings(
    node: &GraphNodeSemanticFact,
    outputs: &BTreeMap<PortAddress, ValueRef>,
    parameters: &mut BTreeMap<PlanParameterHandle, PlanParameterPayload>,
) -> Result<Box<[PlanInputBinding]>, GraphPlanError> {
    node.inputs
        .iter()
        .map(|binding| {
            let source = match &binding.source {
                GraphResolvedInputSource::Output(port) => {
                    PlanInputSource::Value(*outputs.get(port).ok_or(GraphPlanError::InvalidGraph)?)
                }
                GraphResolvedInputSource::Literal(value) => {
                    let identity = format!("input/{}", binding.address);
                    let handle = parameter_handle(identity.clone());
                    parameters.insert(
                        handle.clone(),
                        PlanParameterPayload::new(
                            parameter_schema(identity),
                            protocol_value(&value.value)?,
                        ),
                    );
                    PlanInputSource::Parameter(handle)
                }
            };
            let port = node
                .ports
                .iter()
                .find(|port| port.address == binding.address)
                .ok_or(GraphPlanError::InvalidGraph)?;
            let specialization = node
                .specialization
                .as_ref()
                .ok_or(GraphPlanError::NotReady)?;
            Ok(PlanInputBinding::new(
                plan_port(&binding.address),
                source,
                PlanInputContract {
                    key: match &binding.address.port {
                        yss_graph_document::PortRef::Instance { template, .. } => {
                            template.as_str().into()
                        }
                        yss_graph_document::PortRef::Declared { key } => key.as_str().into(),
                    },
                    group: binding
                        .group
                        .map(|id| PlanInputGroupId::from_existing(id.to_string().into())),
                    expected_type: yss_graph_type_mapping::data_type_from_resolved_type(
                        port.type_state.exact().ok_or(GraphPlanError::NotReady)?,
                    )
                    .ok_or(GraphPlanError::UnsupportedResolvedType)?,
                    coercions: specialization
                        .coercions
                        .iter()
                        .filter(|coercion| coercion.address == binding.address)
                        .map(|coercion| plan_coercion_kind(coercion.kind))
                        .collect(),
                },
            ))
        })
        .collect()
}

fn output_contract(
    port: &GraphPortSemanticFact,
    graph: &PlanGraphId,
) -> Result<PlanOutputContract, GraphPlanError> {
    use yss_data_contract::ValueType;
    use yss_node_protocol::RelationalScalarType;
    Ok(PlanOutputContract {
        data_type: yss_graph_type_mapping::data_type_from_resolved_type(
            port.type_state.exact().ok_or(GraphPlanError::NotReady)?,
        )
        .ok_or(GraphPlanError::UnsupportedResolvedType)?,
        schema: port.schema_state.exact().map(|schema| {
            schema
                .fields
                .iter()
                .map(|field| PlanOutputField {
                    name: field.name.0.clone(),
                    data_type: match field.scalar_type {
                        RelationalScalarType::Known(semantic) => ValueType::Scalar(semantic),
                        RelationalScalarType::Unknown => ValueType::Any,
                    },
                    lineage: field.lineage.as_ref().map(|lineage| PlanFieldLineage {
                        source_identity: lineage.source.clone(),
                        field_identity: lineage.field.clone(),
                    }),
                })
                .collect()
        }),
        category: map_result_category(port.result_category),
        source: PlanSourceIdentity::new(
            graph.clone(),
            Some(PlanNodeId::from_existing(
                port.address.node_id.to_string().into(),
            )),
            Some(plan_port(&port.address)),
        ),
    })
}

fn parameter_value(value: &serde_json::Value) -> Result<PlanParameterValue, GraphPlanError> {
    use serde_json::Value as Json;
    Ok(match value {
        Json::Null => PlanParameterValue::Scalar(PlanParameterScalar::Null),
        Json::Bool(value) => PlanParameterValue::Scalar(PlanParameterScalar::Bool(*value)),
        Json::Number(value) => PlanParameterValue::Scalar(if let Some(value) = value.as_i64() {
            PlanParameterScalar::Integer(value)
        } else if let Some(value) = value.as_u64() {
            PlanParameterScalar::Unsigned(value)
        } else {
            PlanParameterScalar::Decimal(CanonicalDecimal::try_new(
                value.as_f64().ok_or(GraphPlanError::InvalidLiteral)?,
            )?)
        }),
        Json::String(value) => {
            PlanParameterValue::Scalar(PlanParameterScalar::String(value.clone().into()))
        }
        Json::Array(values) => PlanParameterValue::List(
            values
                .iter()
                .map(parameter_value)
                .collect::<Result<Box<[_]>, _>>()?,
        ),
        Json::Object(values) => PlanParameterValue::Record(
            values
                .iter()
                .map(|(key, value)| {
                    Ok((
                        KernelParameterKey::new(key.clone().into())?,
                        parameter_value(value)?,
                    ))
                })
                .collect::<Result<_, GraphPlanError>>()?,
        ),
    })
}

fn protocol_value(value: &Value) -> Result<PlanParameterValue, GraphPlanError> {
    Ok(match value {
        Value::Null => PlanParameterValue::Scalar(PlanParameterScalar::Null),
        Value::Bool(value) => PlanParameterValue::Scalar(PlanParameterScalar::Bool(*value)),
        Value::Integer(value) => PlanParameterValue::Scalar(PlanParameterScalar::Integer(*value)),
        Value::Unsigned(value) => PlanParameterValue::Scalar(PlanParameterScalar::Unsigned(*value)),
        Value::Decimal(value) => {
            PlanParameterValue::Scalar(PlanParameterScalar::Decimal(CanonicalDecimal::try_new(
                value
                    .as_str()
                    .parse::<f64>()
                    .map_err(|_| GraphPlanError::InvalidLiteral)?,
            )?))
        }
        Value::String(value) => {
            PlanParameterValue::Scalar(PlanParameterScalar::String(value.clone()))
        }
        Value::Bytes(values) => PlanParameterValue::List(
            values
                .iter()
                .map(|value| {
                    PlanParameterValue::Scalar(PlanParameterScalar::Unsigned(u64::from(*value)))
                })
                .collect(),
        ),
        Value::List(values) => PlanParameterValue::List(
            values
                .iter()
                .map(protocol_value)
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(values) => PlanParameterValue::Record(
            values
                .iter()
                .map(|(key, value)| {
                    Ok((
                        KernelParameterKey::new(key.clone())?,
                        protocol_value(value)?,
                    ))
                })
                .collect::<Result<_, GraphPlanError>>()?,
        ),
    })
}

fn plan_specialization(
    value: &yss_graph_analysis::GraphKernelSpecialization,
) -> Result<PlanKernelSpecialization, GraphPlanError> {
    let implementation =
        KernelId::new(value.implementation.clone()).map_err(GraphPlanError::KernelIdentity)?;
    let bindings = |values: &[yss_graph_analysis::GraphPortTypeBinding]| {
        values
            .iter()
            .map(|binding| {
                let port = PlanPortAddress::new(binding.address.to_string().into_boxed_str())
                    .map_err(GraphPlanError::Identity)?;
                let data_type =
                    yss_graph_type_mapping::data_type_from_resolved_type(&binding.value_type)
                        .ok_or(GraphPlanError::UnsupportedResolvedType)?;
                Ok(PlanTypeBinding::new(port, data_type))
            })
            .collect::<Result<Vec<_>, GraphPlanError>>()
            .map(Vec::into_boxed_slice)
    };
    let coercions = value
        .coercions
        .iter()
        .map(|coercion| {
            let port = PlanPortAddress::new(coercion.address.to_string().into_boxed_str())
                .map_err(GraphPlanError::Identity)?;
            let kind = plan_coercion_kind(coercion.kind);
            Ok(PlanInputCoercion::new(port, kind))
        })
        .collect::<Result<Vec<_>, GraphPlanError>>()?
        .into_boxed_slice();
    Ok(PlanKernelSpecialization::new(
        implementation,
        bindings(&value.input_types)?,
        bindings(&value.output_types)?,
        coercions,
    ))
}

fn plan_coercion_kind(kind: yss_node_protocol::InputCoercionKind) -> PlanInputCoercionKind {
    match kind {
        yss_node_protocol::InputCoercionKind::BroadcastScalarToSeries => {
            PlanInputCoercionKind::BroadcastScalarToSeries
        }
    }
}

fn constant_runtime_value(
    constant: &yss_graph_document::GraphConstant,
) -> Result<yss_node_kernel::RuntimeValue, GraphPlanError> {
    use yss_node_kernel::RuntimeValue;
    let Some(snapshot) = &constant.tabular else {
        return RuntimeValue::try_from(&constant.data_value).map_err(GraphPlanError::ConstantValue);
    };
    let column_value = |column: &yss_tabular_contract::TabularColumn| {
        RuntimeValue::List(
            column
                .values()
                .iter()
                .cloned()
                .map(RuntimeValue::from)
                .collect(),
        )
    };
    if matches!(
        constant.data_type,
        yss_data_contract::ValueType::DataSeries(_)
    ) {
        return snapshot
            .columns()
            .first()
            .map(column_value)
            .ok_or(GraphPlanError::UnsupportedResolvedType);
    }
    Ok(RuntimeValue::Record(std::sync::Arc::new(
        snapshot
            .columns()
            .iter()
            .map(|column| (column.name().as_str().into(), column_value(column)))
            .collect(),
    )))
}

fn map_result_category(category: GraphResultCategory) -> crate::plan::ResultCategory {
    use crate::plan::{PlotDataKind, ResultCategory, StatisticalReportKind};
    match category {
        GraphResultCategory::Value => ResultCategory::Value,
        GraphResultCategory::PlotData(kind) => ResultCategory::PlotData(match kind {
            GraphPlotDataKind::Scatter => PlotDataKind::Scatter,
            GraphPlotDataKind::Line => PlotDataKind::Line,
            GraphPlotDataKind::Plot => PlotDataKind::Plot,
            GraphPlotDataKind::Ecdf => PlotDataKind::Ecdf,
            GraphPlotDataKind::Kde => PlotDataKind::Kde,
            GraphPlotDataKind::Histogram => PlotDataKind::Histogram,
            GraphPlotDataKind::Correlation => PlotDataKind::Correlation,
            GraphPlotDataKind::Correlogram => PlotDataKind::Correlogram,
        }),
        GraphResultCategory::StatisticalReport(kind) => {
            ResultCategory::StatisticalReport(match kind {
                GraphStatisticalReportKind::LinearRegressionSummary => {
                    StatisticalReportKind::LinearRegressionSummary
                }
                GraphStatisticalReportKind::BinarySummary => StatisticalReportKind::BinarySummary,
                GraphStatisticalReportKind::Iv2slsSummary => StatisticalReportKind::Iv2slsSummary,
                GraphStatisticalReportKind::IvLimlSummary => StatisticalReportKind::IvLimlSummary,
                GraphStatisticalReportKind::PraisSummary => StatisticalReportKind::PraisSummary,
                GraphStatisticalReportKind::VarSummary => StatisticalReportKind::VarSummary,
                GraphStatisticalReportKind::VarSoc => StatisticalReportKind::VarSoc,
                GraphStatisticalReportKind::PanelSummary => StatisticalReportKind::PanelSummary,
                GraphStatisticalReportKind::PanelDid => StatisticalReportKind::PanelDid,
                GraphStatisticalReportKind::DfAdfSummary => StatisticalReportKind::DfAdfSummary,
                GraphStatisticalReportKind::DfAdfSummaryList => {
                    StatisticalReportKind::DfAdfSummaryList
                }
                GraphStatisticalReportKind::VecSummary => StatisticalReportKind::VecSummary,
                GraphStatisticalReportKind::VecRankSummary => StatisticalReportKind::VecRankSummary,
            })
        }
    }
}

#[cfg(test)]
mod tests;
