pub(crate) use super::super::KernelFragment;
use crate::node_system::runtime::{KernelError, RuntimeValue};

pub(crate) fn expect_arity(inputs: &[RuntimeValue], expected: usize) -> Result<(), KernelError> {
    if inputs.len() == expected {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "kernel received {} inputs; expected {expected}",
            inputs.len()
        )))
    }
}

pub(crate) fn expect_min_arity(inputs: &[RuntimeValue], minimum: usize) -> Result<(), KernelError> {
    if inputs.len() >= minimum {
        Ok(())
    } else {
        Err(KernelError::new(format!(
            "kernel received {} inputs; expected at least {minimum}",
            inputs.len()
        )))
    }
}
