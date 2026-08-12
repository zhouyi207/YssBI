use super::support::{KernelFragment, expect_arity};
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{ArtifactKind, Kernel, KernelContext, KernelError, RuntimeValue};

pub(super) fn register(fragment: &mut KernelFragment) {
    fragment.register("yssbi.debug.print", PrintKernel);
    fragment.register("yssbi.debug.view", ViewKernel);
}

struct PrintKernel;

impl Kernel for PrintKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let RuntimeValue::Scalar(Value::String(message)) = &inputs[0] else {
            return Err(KernelError::new("Print message must be a String scalar"));
        };
        crate::log::emit_execution_log(
            crate::log::LogLevel::Info,
            message.to_string(),
            Some(format!(
                "yssbi.debug.print activation={}",
                context.activation_id.get()
            )),
        );
        Ok(Vec::new())
    }
}

struct ViewKernel;

impl Kernel for ViewKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let artifact = match &inputs[0] {
            RuntimeValue::Scalar(value) => context
                .resource_owner
                .materialize_artifact(ArtifactKind::Replayable, std::iter::once(Ok(value.clone()))),
            RuntimeValue::Artifact(artifact) => {
                let cursor = artifact
                    .cursor()
                    .map_err(|error| KernelError::new(error.to_string()))?;
                context
                    .resource_owner
                    .materialize_artifact(ArtifactKind::Replayable, cursor)
            }
            RuntimeValue::Stream(_) => {
                return Err(KernelError::new("View requires a fully materialized input"));
            }
        }
        .map_err(|error| KernelError::new(error.to_string()))?;
        Ok(vec![RuntimeValue::Artifact(artifact)])
    }
}
