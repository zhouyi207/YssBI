use super::support::{KernelFragment, expect_arity};
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{
    Artifact, ArtifactKind, Kernel, KernelContext, KernelError, RuntimeValue,
};

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
        tauri_plugin_log::log::info!(
            target: "yssbi::node_system::debug",
            "Print [activation={}]: {}",
            context.activation_id.get(),
            message,
        );
        Ok(Vec::new())
    }
}

struct ViewKernel;

impl Kernel for ViewKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let values = match &inputs[0] {
            RuntimeValue::Scalar(value) => vec![value.clone()],
            RuntimeValue::Artifact(artifact) => artifact.values().to_vec(),
            RuntimeValue::Stream(_) => {
                return Err(KernelError::new("View requires a fully materialized input"));
            }
        };
        Ok(vec![RuntimeValue::Artifact(Artifact::new(
            ArtifactKind::Replayable,
            values,
        ))])
    }
}
