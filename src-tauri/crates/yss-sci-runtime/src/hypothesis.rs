//! Backend-neutral hypothesis computation entry points.
use std::collections::HashMap;
use yss_sci_contract::hypothesis::{HypothesisError, HypothesisTestInput, HypothesisTestOutput};

pub fn run_hypothesis_test(
    input: HypothesisTestInput,
) -> Result<HypothesisTestOutput, HypothesisError> {
    yss_sci::stats::linear_hypothesis::run_hypothesis_test(input)
}

pub fn parse_at_values(
    at_spec: &str,
    param_names: &[String],
) -> Result<HashMap<String, f64>, HypothesisError> {
    yss_sci::stats::linear_hypothesis::parse_at_values(at_spec, param_names)
}
