pub(crate) mod basis;
pub mod identity;
pub(crate) mod model;
pub(crate) mod observation;
pub(crate) mod package;
pub(crate) mod parameter;
pub(crate) mod validation;

pub use basis::{
    PlanCompilationBasis, PlanResourceObservationSet, PlanResourceObservedState,
    PlanResourceRequirement, PlanResourceVersionSet, ResourceAccess, ResourceKind,
};
pub use identity::{
    InvalidPlanIdentity, PlanCompileId, PlanFunctionParameterId, PlanGraphId, PlanGraphRevision,
    PlanNodeId, PlanOutputRef, PlanPortAddress, PlanProjectSessionId, PlanProvenance,
    PlanRegistryFingerprint, PlanResourceId, PlanResourceVersion, PlanSourceIdentity,
};
pub use model::{ExecutionPlan, FunctionPlanAbi, PlanOperation};
pub use observation::{PlanObservationIntent, ValueRef};
pub use package::{CompiledExecutionPackage, CompiledFunctionBundle, CompiledFunctionPlan};
pub use parameter::{
    CanonicalDecimal, CanonicalDecimalError, CompiledParameterBundle,
    CompiledParameterBundleBuilder, CompiledParameterBundleError, CompiledParameterHandle,
    InvalidPlanParameterId, PlanParameterFieldId, PlanParameterPayload, PlanParameterScalar,
    PlanParameterSchemaId, PlanParameterValue,
};
pub use validation::{PlanValidationError, PlanValidationErrors};

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn basis() -> PlanCompilationBasis {
        PlanCompilationBasis::new(
            PlanProjectSessionId::from_existing("session".into()),
            PlanGraphRevision::from_existing(1),
            PlanRegistryFingerprint::from_bytes([1; 32]),
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
        let mut builder = CompiledParameterBundleBuilder::new(basis());
        let handle = CompiledParameterHandle::from_existing("parameter".into());
        let payload = PlanParameterPayload::new(
            PlanParameterSchemaId::from_existing("schema".into()),
            PlanParameterValue::Scalar(PlanParameterScalar::Null),
        );
        builder.insert(handle.clone(), payload.clone()).unwrap();
        assert_eq!(
            builder.insert(handle.clone(), payload),
            Err(CompiledParameterBundleError::DuplicateHandle { handle })
        );
    }
}
