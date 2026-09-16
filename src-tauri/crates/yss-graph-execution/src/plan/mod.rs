pub(crate) mod basis;
pub mod identity;
pub(crate) mod model;
pub(crate) mod observation;
pub(crate) mod package;
pub(crate) mod parameter;
pub(crate) mod validation;

pub(crate) mod result_category;

pub use basis::{
    PlanBasis, PlanResourceObservationSet, PlanResourceObservedState, PlanResourceRequirement,
    PlanResourceVersionSet, ResourceAccess, ResourceKind,
};
pub use identity::{
    InvalidPlanIdentity, PlanGraphId, PlanId, PlanInputGroupId, PlanNodeId, PlanNodeTypeId,
    PlanOutputRef, PlanPortAddress, PlanProjectSessionId, PlanProvenance, PlanRegistryFingerprint,
    PlanResourceId, PlanResourceVersion, PlanSourceIdentity,
};
pub use model::{
    ExecutionPlan, PlanExecutionDemand, PlanFieldLineage, PlanInputBinding, PlanInputCoercion,
    PlanInputCoercionKind, PlanInputContract, PlanInputSource, PlanKernelSpecialization,
    PlanOperation, PlanOutputBinding, PlanOutputContract, PlanOutputField, PlanTypeBinding,
};
pub use observation::{PlanObservationIntent, ValueRef};
pub use package::ExecutionPlanPackage;
pub use parameter::{
    CanonicalDecimal, CanonicalDecimalError, InvalidPlanParameterId, PlanParameterBundle,
    PlanParameterBundleBuilder, PlanParameterBundleError, PlanParameterHandle,
    PlanParameterPayload, PlanParameterScalar, PlanParameterSchemaId, PlanParameterValue,
};
pub use result_category::{PlotDataKind, ResultCategory, StatisticalReportKind};
pub use validation::PlanValidationError;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn basis() -> PlanBasis {
        PlanBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanRegistryFingerprint::from_bytes([1; 32]),
            yss_node_kernel::KernelRegistry::default().fingerprint(),
            BTreeMap::new(),
            BTreeMap::new(),
        )
    }

    #[test]
    fn parameter_and_session_identities_reject_empty_or_whitespace() {
        assert_eq!(
            PlanProjectSessionId::new("".into()),
            Err(InvalidPlanIdentity::Empty)
        );
        assert_eq!(
            PlanProjectSessionId::new(" session".into()),
            Err(InvalidPlanIdentity::SurroundingWhitespace)
        );
        assert_eq!(
            PlanParameterSchemaId::new(" ".into()),
            Err(InvalidPlanParameterId::SurroundingWhitespace)
        );
    }

    #[test]
    fn duplicate_parameter_handles_are_rejected_before_freezing() {
        let mut builder = PlanParameterBundleBuilder::new(basis());
        let handle = PlanParameterHandle::from_existing("parameter".into());
        let payload = PlanParameterPayload::new(
            PlanParameterSchemaId::from_existing("schema".into()),
            PlanParameterValue::Scalar(PlanParameterScalar::Null),
        );
        builder.insert(handle.clone(), payload.clone()).unwrap();
        assert_eq!(
            builder.insert(handle.clone(), payload),
            Err(PlanParameterBundleError::DuplicateHandle { handle })
        );
    }
}
