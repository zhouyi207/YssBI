use serde::Deserialize;
use std::time::{Duration, Instant};
use yss_sci_contract::scientific::{
    AcfPacfRequest, AcfPacfResult, ScientificCancellationToken, ScientificExecutionControl,
};
use yss_sci_runtime::acf_pacf;

const SIMPLE_EXPONENTIAL: &str =
    include_str!("fixtures/time_series/acf_pacf/simple_exponential.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcfPacfGoldenFixture {
    name: String,
    input: FixtureInput,
    expected: ExpectedOutput,
    tolerance: Tolerance,
}

#[derive(Debug, Clone, Deserialize)]
struct FixtureInput {
    residuals: Vec<f64>,
    max_lag: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ExpectedOutput {
    acf: Vec<f64>,
    pacf: Vec<f64>,
    n: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct Tolerance {
    absolute: f64,
}

#[test]
fn rust_acf_pacf_matches_golden_fixtures() {
    for fixture in fixtures() {
        let result = acf_pacf(
            AcfPacfRequest {
                values: fixture.input.residuals.clone(),
                max_lag: fixture.input.max_lag,
            },
            &ScientificExecutionControl {
                cancellation: ScientificCancellationToken::new(),
                deadline: Instant::now() + Duration::from_secs(5),
            },
        )
        .unwrap_or_else(|error| panic!("{} rust acf/pacf failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &result,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }
}

fn fixtures() -> Vec<AcfPacfGoldenFixture> {
    vec![parse_fixture(SIMPLE_EXPONENTIAL)]
}

fn parse_fixture(contents: &str) -> AcfPacfGoldenFixture {
    serde_json::from_str(contents).expect("valid ACF/PACF golden fixture")
}

fn assert_output_close(
    name: &str,
    actual: &AcfPacfResult,
    expected: &ExpectedOutput,
    tolerance: f64,
) {
    assert_eq!(actual.n, expected.n, "{name} n");
    assert_close_slice(name, "acf", &actual.acf, &expected.acf, tolerance);
    assert_close_slice(name, "pacf", &actual.pacf, &expected.pacf, tolerance);
}

fn assert_close_slice(name: &str, metric: &str, actual: &[f64], expected: &[f64], tolerance: f64) {
    assert_eq!(actual.len(), expected.len(), "{name} {metric} length");
    for (index, (actual, expected)) in actual.iter().zip(expected).enumerate() {
        let difference = (actual - expected).abs();
        assert!(
            difference <= tolerance,
            "{name} {metric}[{index}] differs: actual={actual}, expected={expected}, diff={difference}, tolerance={tolerance}"
        );
    }
}
