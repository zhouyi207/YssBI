//! Per-group function invocation reuses private frames and the caller's run control.
use super::{NeutralPlanExecutor, OperationExecutionError, PreparedRunResources, functions};
use crate::plan::*;
use std::collections::BTreeSet;
use std::sync::Arc;
use yss_graph_document::GraphResourcePath;
use yss_node_kernel::{KernelControl, KernelError, RuntimeValue, kernel_error};
use yss_relational_contract::{GroupMapMode, RelationControl, RelationHandle};

pub(super) struct GroupCall<'a> {
    pub operation: &'a PlanOperation,
    pub target: &'a PlanResourceId,
    pub transform: bool,
    pub inputs: &'a [RuntimeValue],
}

pub(super) fn invoke(
    executor: &NeutralPlanExecutor,
    package: &ExecutionPlanPackage,
    resources: &PreparedRunResources,
    control: &KernelControl,
    call: GroupCall<'_>,
    active: &mut BTreeSet<GraphResourcePath>,
) -> Result<Vec<RuntimeValue>, OperationExecutionError> {
    control.check()?;
    let [RuntimeValue::Grouped(groups)] = call.inputs else {
        return Err(KernelError::InputLayoutMismatch.into());
    };
    let mode = if call.transform {
        GroupMapMode::Transform
    } else {
        let handle = call
            .operation
            .parameters()
            .iter()
            .find(|(key, _)| key.as_str() == "key_prefix")
            .map(|(_, handle)| handle)
            .ok_or(KernelError::InvalidParameter)?;
        let parameter = package
            .parameters()
            .entries()
            .get(handle)
            .ok_or(KernelError::InvalidParameter)?;
        let value = crate::kernel_invocation::parameter_value(parameter.value(), resources)?;
        let RuntimeValue::Scalar(yss_data_contract::TabularScalar::String(prefix)) = value.as_ref()
        else {
            return Err(KernelError::InvalidParameter.into());
        };
        GroupMapMode::Apply {
            key_prefix: prefix.clone(),
        }
    };
    let relation_control = RelationControl {
        cancellation: Arc::clone(&control.cancellation),
        deadline: control.deadline,
        max_input_bytes: control.max_input_bytes,
    };
    let mut mapping = groups
        .begin_map(mode, &relation_control)
        .map_err(kernel_error)?;
    let mut count = 0;
    let mut callback =
        |relation: RelationHandle| -> Result<RelationHandle, OperationExecutionError> {
            let returned = functions::invoke(
                executor,
                package,
                resources,
                control,
                functions::FunctionCall {
                    target: call.target,
                    arguments: functions::FunctionArguments::Ordered(&[RuntimeValue::Relation(
                        relation,
                    )]),
                },
                active,
            )?;
            let [RuntimeValue::Relation(result)] = returned.as_slice() else {
                return Err(KernelError::OutputContractMismatch.into());
            };
            Ok(result.clone())
        };
    while let Some(group) = mapping.next(&relation_control).map_err(kernel_error)? {
        let result = callback(group.relation).map_err(|error| {
            error.at_group(call.operation.source(), call.target, Some(group.ordinal))
        })?;
        mapping
            .append(&result, &relation_control)
            .map_err(|error| {
                OperationExecutionError::from(kernel_error(error)).at_group(
                    call.operation.source(),
                    call.target,
                    Some(group.ordinal),
                )
            })?;
        count = group.ordinal;
    }
    let empty = if count == 0 {
        Some(
            callback(groups.source().clone())
                .map_err(|error| error.at_group(call.operation.source(), call.target, None))?,
        )
    } else {
        None
    };
    let result = mapping
        .finish(empty.as_ref(), &relation_control)
        .map_err(|error| {
            let error = OperationExecutionError::from(kernel_error(error));
            if count == 0 {
                error.at_group(call.operation.source(), call.target, None)
            } else {
                error
            }
        })?;
    Ok(vec![RuntimeValue::Relation(result)])
}
