use super::{
    Artifact, ArtifactKind, CancellationToken, RunError, RunId, RunResourceSet, RuntimeValue,
    StreamReceiveError, StreamValue,
};
use crate::node_system::plan::{
    CompiledRelationalPlan, MaterializationBridge, PlannedMaterializationBridge,
    RelationalBackendId, RelationalFragmentId, RelationalSubplan,
};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationalError(pub Box<str>);

impl RelationalError {
    pub fn new(message: impl Into<Box<str>>) -> Self {
        Self(message.into())
    }
}

impl From<RunError> for RelationalError {
    fn from(error: RunError) -> Self {
        Self(error.to_string().into())
    }
}

impl fmt::Display for RelationalError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
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
        if let Some((registered, _)) = &self.backend {
            return Err(RelationalBackendRegistrationError {
                registered: registered.clone(),
                requested: id,
            });
        }
        self.backend = Some((id, Arc::new(backend)));
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
            return Err(RelationalError::new(format!(
                "relational backend '{}' is not registered",
                backend.as_str()
            )));
        };
        if registered != backend {
            return Err(RelationalError::new(format!(
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
                .map_err(|error| RunError::RelationalAcquire {
                    backend: id.clone(),
                    message: error.0,
                })?;
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
                        return Err(RelationalError::new("bridge materialization was cancelled"));
                    }
                    Err(StreamReceiveError::Empty) => unreachable!("blocking receive is not empty"),
                }
            }
        }
    }
}
