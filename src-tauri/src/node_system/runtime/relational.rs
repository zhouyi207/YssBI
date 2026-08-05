use super::{
    Artifact, ArtifactKind, CancellationToken, RunError, RunId, RunResourceSet, RuntimeValue,
    StreamReceiveError, StreamValue,
};
use crate::node_system::plan::{
    CompiledRelationalPlan, MaterializationBridge, PlannedMaterializationBridge,
    RelationalBackendId, RelationalFragmentId, RelationalSubplan,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

pub struct RelationalContext<'a> {
    pub run_id: RunId,
    pub resources: &'a RunResourceSet,
    pub cancellation: &'a CancellationToken,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalInput {
    pub bridge: PlannedMaterializationBridge,
    pub value: RuntimeValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalExecution {
    pub outputs: Vec<RuntimeValue>,
    pub fragment_outputs: BTreeMap<RelationalFragmentId, RuntimeValue>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RelationalErrorCode {
    OperatorInvalid,
    ColumnMissing,
    TypeMismatch,
    InputShapeInvalid,
    HintInvalid,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalError {
    code: RelationalErrorCode,
    message: Box<str>,
}

impl RelationalError {
    pub fn new(code: RelationalErrorCode, message: impl Into<Box<str>>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }

    pub fn operator_invalid(message: impl Into<Box<str>>) -> Self {
        Self::new(RelationalErrorCode::OperatorInvalid, message)
    }

    pub fn cancelled(message: impl Into<Box<str>>) -> Self {
        Self::new(RelationalErrorCode::Cancelled, message)
    }

    pub const fn code(&self) -> RelationalErrorCode {
        self.code
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl From<RunError> for RelationalError {
    fn from(error: RunError) -> Self {
        match error {
            RunError::Cancelled => Self::cancelled("relational execution was cancelled"),
            _ => Self::operator_invalid("relational execution failed"),
        }
    }
}

impl fmt::Display for RelationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for RelationalError {}

pub trait RelationalBackend: Send + Sync {
    fn execute(
        &self,
        context: &RelationalContext<'_>,
        plan: &CompiledRelationalPlan,
        operation_inputs: &[RuntimeValue],
        bridge_inputs: &[RelationalInput],
    ) -> Result<RelationalExecution, RelationalError>;
}

pub trait RelationalBackendLease: Send + Sync {
    fn backend(&self) -> &dyn RelationalBackend;
}

pub trait RelationalBackendProvider: Send + Sync {
    fn acquire(
        &self,
        backend: &RelationalBackendId,
        resources: &RunResourceSet,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn RelationalBackendLease>, RelationalError>;
}

#[derive(Default)]
pub struct RelationalBackendRegistry {
    backend: Option<(RelationalBackendId, Arc<dyn RelationalBackend>)>,
}

impl RelationalBackendRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        id: RelationalBackendId,
        backend: impl RelationalBackend + 'static,
    ) -> Result<(), RelationalBackendRegistrationError> {
        self.register_shared(id, Arc::new(backend))
    }

    #[cfg(test)]
    pub(crate) fn register_shared_for_test(
        &mut self,
        id: RelationalBackendId,
        backend: Arc<dyn RelationalBackend>,
    ) -> Result<(), RelationalBackendRegistrationError> {
        self.register_shared(id, backend)
    }

    fn register_shared(
        &mut self,
        id: RelationalBackendId,
        backend: Arc<dyn RelationalBackend>,
    ) -> Result<(), RelationalBackendRegistrationError> {
        if let Some((registered, _)) = &self.backend {
            return Err(RelationalBackendRegistrationError {
                registered: registered.clone(),
                requested: id,
            });
        }
        self.backend = Some((id, backend));
        Ok(())
    }
}

struct RegistryBackendLease {
    backend: Arc<dyn RelationalBackend>,
}

impl RelationalBackendLease for RegistryBackendLease {
    fn backend(&self) -> &dyn RelationalBackend {
        self.backend.as_ref()
    }
}

impl RelationalBackendProvider for RelationalBackendRegistry {
    fn acquire(
        &self,
        backend: &RelationalBackendId,
        _: &RunResourceSet,
        cancellation: &CancellationToken,
    ) -> Result<Box<dyn RelationalBackendLease>, RelationalError> {
        cancellation.check().map_err(RelationalError::from)?;
        let Some((registered, implementation)) = &self.backend else {
            return Err(RelationalError::operator_invalid(format!(
                "relational backend '{}' is not registered",
                backend.as_str()
            )));
        };
        if registered != backend {
            return Err(RelationalError::operator_invalid(format!(
                "relational backend '{}' is not registered; configured backend is '{}'",
                backend.as_str(),
                registered.as_str()
            )));
        }
        Ok(Box::new(RegistryBackendLease {
            backend: implementation.clone(),
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalBackendRegistrationError {
    pub registered: RelationalBackendId,
    pub requested: RelationalBackendId,
}

impl fmt::Display for RelationalBackendRegistrationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "relational backend '{}' is already configured; cannot register '{}'",
            self.registered.as_str(),
            self.requested.as_str()
        )
    }
}

impl std::error::Error for RelationalBackendRegistrationError {}

pub(crate) struct RunRelationalBackends {
    leases: BTreeMap<RelationalBackendId, Box<dyn RelationalBackendLease>>,
}

impl RunRelationalBackends {
    pub(crate) fn acquire(
        subplans: &[RelationalSubplan],
        provider: Option<&dyn RelationalBackendProvider>,
        resources: &RunResourceSet,
        cancellation: &CancellationToken,
    ) -> Result<Self, RunError> {
        let mut ids = BTreeSet::new();
        for subplan in subplans {
            ids.insert(subplan.backend.clone());
        }
        if ids.is_empty() {
            return Ok(Self {
                leases: BTreeMap::new(),
            });
        }
        let provider = provider.ok_or_else(|| {
            RunError::RelationalBackendNotFound(ids.first().expect("ids is not empty").clone())
        })?;
        let mut leases = BTreeMap::new();
        for id in ids {
            cancellation.check()?;
            let lease = provider
                .acquire(&id, resources, cancellation)
                .map_err(|error| RunError::from_relational_acquire(id.clone(), error))?;
            leases.insert(id, lease);
        }
        Ok(Self { leases })
    }

    pub(crate) fn get(&self, id: &RelationalBackendId) -> Option<&dyn RelationalBackend> {
        self.leases.get(id).map(|lease| lease.backend())
    }
}

pub fn materialize_bridge(
    bridge: MaterializationBridge,
    value: RuntimeValue,
    cancellation: &CancellationToken,
) -> Result<RuntimeValue, RelationalError> {
    materialize_bridge_inner(
        bridge,
        value,
        cancellation,
        #[cfg(test)]
        None,
    )
}

#[cfg(test)]
pub(crate) fn materialize_bridge_with_checkpoint(
    bridge: MaterializationBridge,
    value: RuntimeValue,
    cancellation: &CancellationToken,
    checkpoint: &dyn Fn(&CancellationToken),
) -> Result<RuntimeValue, RelationalError> {
    materialize_bridge_inner(bridge, value, cancellation, Some(checkpoint))
}

fn materialize_bridge_inner(
    bridge: MaterializationBridge,
    value: RuntimeValue,
    cancellation: &CancellationToken,
    #[cfg(test)] checkpoint: Option<&dyn Fn(&CancellationToken)>,
) -> Result<RuntimeValue, RelationalError> {
    cancellation.check().map_err(RelationalError::from)?;
    if bridge == MaterializationBridge::Stream {
        return match value {
            RuntimeValue::Stream(stream) => Ok(RuntimeValue::Stream(stream)),
            value => Ok(RuntimeValue::Stream(StreamValue::from_values(
                into_values(
                    value,
                    cancellation,
                    #[cfg(test)]
                    checkpoint,
                )?,
                cancellation.clone(),
            )?)),
        };
    }

    let values = into_values(
        value,
        cancellation,
        #[cfg(test)]
        checkpoint,
    )?;
    let kind = match bridge {
        MaterializationBridge::Stream => unreachable!(),
        MaterializationBridge::Buffer => ArtifactKind::Buffered,
        MaterializationBridge::Collect => ArtifactKind::Collected,
        MaterializationBridge::Spill => ArtifactKind::Spilled,
        MaterializationBridge::Replay => ArtifactKind::Replayable,
    };
    Ok(RuntimeValue::Artifact(Artifact::new(kind, values)))
}

fn into_values(
    value: RuntimeValue,
    cancellation: &CancellationToken,
    #[cfg(test)] checkpoint: Option<&dyn Fn(&CancellationToken)>,
) -> Result<Vec<crate::node_system::protocol::Value>, RelationalError> {
    match value {
        RuntimeValue::Scalar(value) => Ok(vec![value]),
        RuntimeValue::Artifact(artifact) => Ok(artifact.values().to_vec()),
        RuntimeValue::Stream(stream) => {
            let mut values = Vec::new();
            loop {
                cancellation.check().map_err(RelationalError::from)?;
                match stream.recv() {
                    Ok(value) => {
                        values.push(value);
                        #[cfg(test)]
                        if let Some(checkpoint) = checkpoint {
                            checkpoint(cancellation);
                        }
                    }
                    Err(StreamReceiveError::Closed) => return Ok(values),
                    Err(StreamReceiveError::Cancelled) => {
                        return Err(RelationalError::cancelled(
                            "bridge materialization was cancelled",
                        ));
                    }
                    Err(StreamReceiveError::Empty) => unreachable!("blocking receive is not empty"),
                }
            }
        }
    }
}

#[cfg(test)]
mod error_tests {
    use super::{RelationalError, RelationalErrorCode};
    use crate::node_system::plan::OperationIndex;
    use crate::node_system::runtime::{RunError, RunErrorCode};

    #[test]
    fn relational_error_codes_have_stable_camel_case_serde() {
        for (code, expected) in [
            (RelationalErrorCode::OperatorInvalid, "\"operatorInvalid\""),
            (RelationalErrorCode::ColumnMissing, "\"columnMissing\""),
            (RelationalErrorCode::TypeMismatch, "\"typeMismatch\""),
            (
                RelationalErrorCode::InputShapeInvalid,
                "\"inputShapeInvalid\"",
            ),
            (RelationalErrorCode::HintInvalid, "\"hintInvalid\""),
            (RelationalErrorCode::Cancelled, "\"cancelled\""),
        ] {
            assert_eq!(serde_json::to_string(&code).unwrap(), expected);
            assert_eq!(
                serde_json::from_str::<RelationalErrorCode>(expected).unwrap(),
                code
            );
        }
    }

    #[test]
    fn relational_error_maps_to_run_error_without_losing_its_code() {
        let error = RelationalError::new(
            RelationalErrorCode::TypeMismatch,
            "filter predicate must be boolean",
        );

        let run_error = RunError::from_relational(OperationIndex::new(3), error);

        assert!(matches!(
            run_error,
            RunError::RelationalFailed {
                operation,
                code: RelationalErrorCode::TypeMismatch,
                ref message,
            } if operation == OperationIndex::new(3)
                && message.as_ref() == "filter predicate must be boolean"
        ));
        assert_eq!(
            RunErrorCode::from(&run_error),
            RunErrorCode::RelationalTypeMismatch
        );
        assert!(
            !serde_json::to_string(&RunErrorCode::from(&run_error))
                .unwrap()
                .contains("filter predicate")
        );
    }

    #[test]
    fn relational_cancellation_remains_typed_run_cancellation() {
        let run_error = RunError::from_relational(
            OperationIndex::new(4),
            RelationalError::cancelled("relational evaluation was cancelled"),
        );

        assert_eq!(run_error, RunError::Cancelled);
        assert_eq!(RunErrorCode::from(&run_error), RunErrorCode::Cancelled);
    }

    #[test]
    fn relational_backend_acquisition_cancellation_remains_typed() {
        struct CancellingProvider;

        impl super::RelationalBackendProvider for CancellingProvider {
            fn acquire(
                &self,
                _: &crate::node_system::plan::RelationalBackendId,
                _: &crate::node_system::runtime::RunResourceSet,
                _: &crate::node_system::runtime::CancellationToken,
            ) -> Result<Box<dyn super::RelationalBackendLease>, RelationalError> {
                Err(RelationalError::cancelled(
                    "backend acquisition was cancelled",
                ))
            }
        }

        let backend =
            crate::node_system::plan::RelationalBackendId::new("relational.test").unwrap();
        let subplan = crate::node_system::plan::RelationalSubplan {
            backend,
            compiled_plan: crate::node_system::plan::CompiledRelationalPlan {
                fragment_order: Box::new([]),
                operators: Box::new([]),
                fragment_roots: Box::new([]),
                bridge_inputs: Box::new([]),
                requested_fragment_outputs: Box::new([]),
                roots: Box::new([]),
                pushdown_hints: Box::new([]),
            },
            materialization_bridges: Box::new([]),
        };
        let result = super::RunRelationalBackends::acquire(
            &[subplan],
            Some(&CancellingProvider),
            &crate::node_system::runtime::RunResourceSet::default(),
            &crate::node_system::runtime::CancellationToken::new(),
        );
        let run_error = match result {
            Ok(_) => panic!("cancelled acquisition unexpectedly succeeded"),
            Err(error) => error,
        };

        assert_eq!(run_error, RunError::Cancelled);
        assert_eq!(RunErrorCode::from(&run_error), RunErrorCode::Cancelled);
    }
}
