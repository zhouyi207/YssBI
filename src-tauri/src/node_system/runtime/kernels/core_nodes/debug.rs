use super::support::{KernelFragment, expect_arity};
use crate::graph::protocol::{PortKey, Value};
use crate::graph_document::PortAddress;
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
        let source_port = PortKey::new("message")
            .map(|key| PortAddress::declared(context.source_node_id(), key))
            .map_err(|_| KernelError::new("Print message port key is invalid"))?;
        context.emit_stdout(message, source_port);
        Ok(Vec::new())
    }
}
