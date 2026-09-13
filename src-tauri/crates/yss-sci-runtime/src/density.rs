//! Runtime entry point for kernel density estimation.
use yss_sci_contract::density::{KernelDensityInput, KernelDensityOutput};

pub fn compute_kernel_density(input: KernelDensityInput<'_>) -> KernelDensityOutput {
    yss_sci::stats::density::compute_kernel_density(input)
}
