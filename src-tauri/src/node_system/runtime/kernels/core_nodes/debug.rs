use super::support::{KernelFragment, expect_arity};
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{Kernel, KernelContext, KernelError, RuntimeValue};

pub(super) fn register(fragment: &mut KernelFragment) {
    fragment.register("yssbi.debug.print", PrintKernel);
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
