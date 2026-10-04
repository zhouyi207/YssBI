//! Call-local arguments and frames; no internal result is installed in the session ResultStore.
mod frame;
use super::{NeutralPlanExecutor, OperationExecutionError, PreparedRunResources};
use crate::plan::*;
use std::collections::{BTreeMap, BTreeSet};
use yss_graph_analysis::{GraphFunctionArgument, GraphSchemaState};
use yss_graph_document::{GraphResourcePath, PortAddress};
use yss_node_kernel::{KernelControl, KernelError, RuntimeValue};
use yss_node_protocol::{ResolvedSchemaFact, SchemaExpr};

pub(super) struct FunctionCall<'a> {
    pub target: &'a PlanResourceId,
    pub arguments: FunctionArguments<'a>,
}

pub(super) enum FunctionArguments<'a> {
    Ports {
        operation: &'a PlanOperation,
        bindings: &'a [(Box<str>, PlanPortAddress)],
        values: &'a [RuntimeValue],
    },
    Ordered(&'a [RuntimeValue]),
}

impl FunctionArguments<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Ports { values, .. } | Self::Ordered(values) => values.len(),
        }
    }
    fn value(&self, id: &str, ordinal: usize) -> Option<&RuntimeValue> {
        match self {
            Self::Ports {
                operation,
                bindings,
                values,
            } => {
                if bindings.len() != values.len() {
                    return None;
                }
                let (_, port) = bindings.iter().find(|(key, _)| key.as_ref() == id)?;
                let index = operation
                    .inputs()
                    .iter()
                    .position(|input| input.port() == port)?;
                values.get(index)
            }
            Self::Ordered(values) => values.get(ordinal),
        }
    }
}

pub(super) fn invoke(
    executor: &NeutralPlanExecutor,
    package: &ExecutionPlanPackage,
    resources: &PreparedRunResources,
    control: &KernelControl,
    call: FunctionCall<'_>,
    active: &mut BTreeSet<GraphResourcePath>,
) -> Result<Vec<RuntimeValue>, OperationExecutionError> {
    let FunctionCall {
        target,
        arguments: supplied,
    } = call;
    control.check()?;
    let path =
        GraphResourcePath::new(target.as_str()).map_err(|_| OperationExecutionError::Failed)?;
    let library = package
        .functions
        .as_ref()
        .ok_or(OperationExecutionError::Failed)?;
    let definition = library
        .definitions
        .get(&path)
        .ok_or(OperationExecutionError::Failed)?;
    if definition.state == yss_graph_analysis::GraphFunctionState::Invalid
        || !active.insert(path.clone())
    {
        return Err(KernelError::InvalidParameter.into());
    }
    let result = (|| {
        if supplied.len() != definition.abi.parameters.len() {
            return Err(KernelError::InputLayoutMismatch.into());
        }
        let mut arguments = BTreeMap::new();
        let mut values = BTreeMap::new();
        for (ordinal, parameter) in definition.abi.parameters.iter().enumerate() {
            let expected =
                yss_graph_type_mapping::data_type_from_resolved_type(&parameter.value_type)
                    .ok_or(KernelError::InputLayoutMismatch)?;
            let mut value = supplied
                .value(parameter.id.as_str(), ordinal)
                .ok_or(KernelError::InputLayoutMismatch)?
                .clone();
            if !value.matches_carrier(&expected) {
                return Err(KernelError::InputLayoutMismatch.into());
            }
            if matches!(&value, RuntimeValue::Relation(relation) if relation.schema_is_deferred()) {
                super::results::stabilize_outputs(
                    std::slice::from_mut(&mut value),
                    &[true],
                    &executor.relations,
                    control,
                )?;
            }
            let schema = match schema_fields(&value) {
                Some(fields) => GraphSchemaState::Exact(ResolvedSchemaFact {
                    expression: SchemaExpr::Fixed {
                        fields: fields.clone(),
                    },
                    fields,
                }),
                None => GraphSchemaState::NotApplicable,
            };
            arguments.insert(
                parameter.id.clone(),
                GraphFunctionArgument {
                    value_type: parameter.value_type.clone(),
                    schema,
                },
            );
            values.insert(output_ref(&path, &parameter.entry_output), value);
        }
        frame::execute(
            executor,
            package,
            resources,
            control,
            frame::FunctionFrame {
                path: &path,
                definition,
                arguments: &arguments,
                entry_values: &values,
            },
            active,
        )
    })();
    active.remove(&path);
    result
}

fn output_ref(path: &GraphResourcePath, port: &PortAddress) -> PlanOutputRef {
    PlanOutputRef::new(
        PlanGraphId::from_existing(path.as_str().into()),
        PlanPortAddress::from_existing(port.to_string().into()),
    )
}

fn schema_fields(value: &RuntimeValue) -> Option<Vec<yss_node_protocol::SchemaField>> {
    let RuntimeValue::Relation(relation) = value.unannotated() else {
        return None;
    };
    if relation.schema_is_deferred() {
        return None;
    }
    Some(
        relation
            .schema()
            .fields()
            .iter()
            .map(|field| yss_node_protocol::SchemaField {
                name: yss_node_protocol::SchemaColumnRef(field.name().as_str().into()),
                scalar_type: yss_database_arrow::column_semantic(field)
                    .map(|semantic| yss_node_protocol::RelationalScalarType::Known(semantic.kind))
                    .unwrap_or(yss_node_protocol::RelationalScalarType::Unknown),
                lineage: None,
            })
            .collect(),
    )
}
