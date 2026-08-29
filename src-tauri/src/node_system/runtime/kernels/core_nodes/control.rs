use super::support::{KernelFragment, expect_arity};
use crate::graph::protocol::Value;
use crate::node_system::runtime::{Kernel, KernelContext, KernelError, RuntimeValue};
use std::time::Duration;

const MAX_SLEEP_SECONDS: f64 = 60.0;

pub(super) fn register(fragment: &mut KernelFragment) {
    fragment.register("yssbi.control.do", DoKernel);
    fragment.register("yssbi.control.sleep", SleepKernel);
}

struct DoKernel;

impl Kernel for DoKernel {
    fn execute(
        &self,
        _context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 0)?;
        Ok(Vec::new())
    }
}

struct SleepKernel;

impl Kernel for SleepKernel {
    fn execute(
        &self,
        context: &KernelContext<'_>,
        inputs: &[RuntimeValue],
    ) -> Result<Vec<RuntimeValue>, KernelError> {
        expect_arity(inputs, 1)?;
        let RuntimeValue::Scalar(Value::Decimal(duration)) = &inputs[0] else {
            return Err(KernelError::new("Sleep duration must be a Float64 scalar"));
        };
        let seconds = duration
            .as_str()
            .parse::<f64>()
            .map_err(|_| KernelError::new("Sleep duration is not a valid Float64"))?;
        if !seconds.is_finite() || !(0.0..=MAX_SLEEP_SECONDS).contains(&seconds) {
            return Err(KernelError::new(
                "Sleep duration must be between zero and sixty seconds",
            ));
        }

        context.wait_for(Duration::from_secs_f64(seconds))?;
        Ok(Vec::new())
    }
}
