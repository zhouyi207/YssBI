use super::support::{KernelFragment, expect_arity};
use crate::graph::protocol::Value;
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
        context.emit_stdout(message);
        Ok(Vec::new())
    }
}
