//! Lower one prepared graph operation into a graph-independent kernel call.

use std::borrow::Cow;
use std::collections::BTreeMap;

use yss_node_kernel::{
    KernelControl, KernelError, KernelField, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

use crate::plan::{PlanOperation, PlanParameterBundle, PlanParameterScalar, PlanParameterValue};
use crate::resource_preparation::PreparedRunResources;

pub(crate) fn invoke(
    kernels: &KernelRegistry,
    relations: &std::sync::Arc<dyn yss_relational_contract::RelationFactory>,
    operation: &PlanOperation,
    inputs: &[RuntimeValue],
    parameters: &PlanParameterBundle,
    resources: &PreparedRunResources,
    control: &KernelControl,
) -> Result<Vec<RuntimeValue>, KernelError> {
    let input_keys = operation
        .inputs()
        .iter()
        .map(|input| input.contract().key.as_ref())
        .collect::<Vec<_>>();
    let outputs = operation
        .outputs()
        .iter()
        .map(|output| {
            let contract = output.contract();
            KernelOutputSpec {
                data_type: contract.data_type.clone(),
                fields: contract.schema.as_ref().map(|fields| {
                    fields
                        .iter()
                        .map(|field| KernelField {
                            name: field.name.clone(),
                            data_type: field.data_type.clone(),
                        })
                        .collect()
                }),
            }
        })
        .collect::<Vec<_>>();
    let parameters = operation
        .parameters()
        .iter()
        .map(|(key, handle)| {
            let payload = parameters
                .entries()
                .get(handle)
                .ok_or(KernelError::Failed)?;
            Ok((key.clone(), parameter_value(payload.value(), resources)?))
        })
        .collect::<Result<BTreeMap<_, _>, KernelError>>()?;
    kernels.execute(
        operation.kernel_id(),
        &KernelInvocation {
            relations,
            inputs,
            input_keys: &input_keys,
            parameters,
            outputs: &outputs,
            control,
        },
    )
}

/// Resolve resource references only against the run's prepared, authorized bindings.
pub(crate) fn parameter_value<'a>(
    value: &'a PlanParameterValue,
    resources: &'a PreparedRunResources,
) -> Result<Cow<'a, RuntimeValue>, KernelError> {
    Ok(match value {
        PlanParameterValue::Scalar(value) => Cow::Owned(match value {
            PlanParameterScalar::Null => RuntimeValue::Null,
            PlanParameterScalar::Bool(value) => RuntimeValue::Bool(*value),
            PlanParameterScalar::Integer(value) => RuntimeValue::Integer(*value),
            PlanParameterScalar::Unsigned(value) => RuntimeValue::Unsigned(*value),
            PlanParameterScalar::Decimal(value) => RuntimeValue::Decimal(value.value()),
            PlanParameterScalar::String(value) => RuntimeValue::String(value.clone()),
        }),
        PlanParameterValue::Literal(value) => Cow::Borrowed(value.as_ref()),
        PlanParameterValue::Resource(resource) => {
            Cow::Borrowed(resources.value(resource).ok_or(KernelError::Failed)?)
        }
        PlanParameterValue::List(values) => Cow::Owned(RuntimeValue::List(
            values
                .iter()
                .map(|value| parameter_value(value, resources).map(Cow::into_owned))
                .collect::<Result<std::sync::Arc<[_]>, _>>()?,
        )),
        PlanParameterValue::Record(fields) => {
            Cow::Owned(RuntimeValue::Record(std::sync::Arc::new(
                fields
                    .iter()
                    .map(|(key, value)| {
                        Ok((
                            key.as_str().into(),
                            parameter_value(value, resources)?.into_owned(),
                        ))
                    })
                    .collect::<Result<BTreeMap<_, _>, KernelError>>()?,
            )))
        }
    })
}
