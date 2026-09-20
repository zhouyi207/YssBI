use yss_sci_contract::distribution::{SampleValue, SamplingDistribution};
use yss_sci_contract::scientific::{ScientificComputationError, ScientificExecutionControl};

/// The caller admits output memory before allocating this slice. No intermediate sample buffer.
pub fn sample_into(
    distribution: SamplingDistribution,
    output: &mut [SampleValue],
    control: &ScientificExecutionControl,
) -> Result<(), ScientificComputationError> {
    yss_sci::distribution::sample_into(distribution, output, control)
}
