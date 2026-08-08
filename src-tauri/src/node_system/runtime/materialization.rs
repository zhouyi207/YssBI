use super::{
    Artifact, ArtifactKind, CancellationToken, RunError, RuntimeValue, StreamReceiveError,
    StreamValue,
};
use crate::node_system::plan::PlannedAdapter;

pub fn execute_planned_adapter(
    adapter: &PlannedAdapter,
    value: RuntimeValue,
    cancellation: &CancellationToken,
) -> Result<RuntimeValue, RunError> {
    cancellation.check()?;
    match adapter {
        PlannedAdapter::Identity => Ok(value),
        PlannedAdapter::StreamBridge { .. } => match value {
            RuntimeValue::Stream(stream) => Ok(RuntimeValue::Stream(stream)),
            value => Ok(RuntimeValue::Stream(StreamValue::from_values(
                into_values(value, cancellation)?,
                cancellation.clone(),
            )?)),
        },
        PlannedAdapter::Buffer { .. } => artifact(ArtifactKind::Buffered, value, cancellation),
        PlannedAdapter::Collect { .. } => artifact(ArtifactKind::Collected, value, cancellation),
        PlannedAdapter::Spill { .. } => artifact(ArtifactKind::Spilled, value, cancellation),
        PlannedAdapter::Replay => artifact(ArtifactKind::Replayable, value, cancellation),
    }
}

fn artifact(
    kind: ArtifactKind,
    value: RuntimeValue,
    cancellation: &CancellationToken,
) -> Result<RuntimeValue, RunError> {
    Ok(RuntimeValue::Artifact(Artifact::new(
        kind,
        into_values(value, cancellation)?,
    )))
}

fn into_values(
    value: RuntimeValue,
    cancellation: &CancellationToken,
) -> Result<Vec<crate::node_system::protocol::Value>, RunError> {
    match value {
        RuntimeValue::Scalar(value) => Ok(vec![value]),
        RuntimeValue::Artifact(artifact) => Ok(artifact.values().to_vec()),
        RuntimeValue::Stream(stream) => {
            let mut values = Vec::new();
            loop {
                cancellation.check()?;
                match stream.recv() {
                    Ok(value) => values.push(value),
                    Err(StreamReceiveError::Closed) => return Ok(values),
                    Err(StreamReceiveError::Cancelled) => return Err(RunError::Cancelled),
                    Err(StreamReceiveError::Empty) => unreachable!("blocking receive is not empty"),
                }
            }
        }
    }
}
