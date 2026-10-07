//! Backend-neutral hypothesis computation entry points.
use std::collections::HashMap;
use yss_sci_contract::execution::ScientificExecutionControl;
use yss_sci_contract::hypothesis::{HypothesisError, HypothesisTestInput, HypothesisTestOutput};

pub fn sample_mean_test(
    input: yss_sci_contract::hypothesis::ClassicalHypothesisTest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::hypothesis::ClassicalTestResult, HypothesisError> {
    yss_sci::hypothesis::sample_mean::run(input, control)
}

pub fn categorical_test(
    input: yss_sci_contract::hypothesis::CategoricalHypothesisTest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::hypothesis::ClassicalTestResult, HypothesisError> {
    yss_sci::hypothesis::categorical::run(input, control)
}

pub fn rank_test(
    input: yss_sci_contract::hypothesis::RankHypothesisTest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::hypothesis::ClassicalTestResult, HypothesisError> {
    yss_sci::hypothesis::nonparametric::run(input, control)
}

pub fn variance_test(
    input: yss_sci_contract::hypothesis::VarianceHomogeneityTest,
    control: &ScientificExecutionControl,
) -> Result<yss_sci_contract::hypothesis::ClassicalTestResult, HypothesisError> {
    yss_sci::hypothesis::variance::run(input, control)
}

pub fn run_hypothesis_test(
    input: HypothesisTestInput,
) -> Result<HypothesisTestOutput, HypothesisError> {
    yss_sci::hypothesis::linear_hypothesis::run_hypothesis_test(input)
}

pub fn parse_at_values(
    at_spec: &str,
    param_names: &[String],
) -> Result<HashMap<String, f64>, HypothesisError> {
    yss_sci::hypothesis::linear_hypothesis::parse_at_values(at_spec, param_names)
}

pub use yss_sci::hypothesis::linear_hypothesis::run_asymptotic_hypothesis_test;
