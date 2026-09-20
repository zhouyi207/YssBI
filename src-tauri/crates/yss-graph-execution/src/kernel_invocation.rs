//! Lower one prepared graph operation into a graph-independent kernel call.

use std::borrow::Cow;
use std::collections::BTreeMap;

use yss_node_kernel::{
    KernelControl, KernelError, KernelField, KernelInvocation, KernelOutputSpec, KernelRegistry,
    RuntimeValue,
};

use crate::plan::{PlanOperation, PlanParameterBundle, PlanParameterValue};
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
        PlanParameterValue::Literal(value) => Cow::Borrowed(value.as_ref()),
        PlanParameterValue::Resource(resource) => {
            Cow::Borrowed(resources.value(resource).ok_or(KernelError::Failed)?)
        }
    })
}
