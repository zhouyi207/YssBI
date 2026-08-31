use serde::Deserialize;
use yss_sci_runtime::api::time_series::acf_pacf::{AcfPacfInput, AcfPacfOutput, compute_acf_pacf};

const SIMPLE_EXPONENTIAL: &str =
    include_str!("fixtures/time_series/acf_pacf/simple_exponential.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AcfPacfGoldenFixture {
    name: String,
    input: AcfPacfInput,
    expected: AcfPacfOutput,
    tolerance: Tolerance,
}

#[derive(Debug, Clone, Deserialize)]
struct Tolerance {
    absolute: f64,
}

#[test]
fn rust_acf_pacf_matches_golden_fixtures() {
    for fixture in fixtures() {
        let result = compute_acf_pacf(fixture.input.clone())
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
    actual: &AcfPacfOutput,
    expected: &AcfPacfOutput,
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
