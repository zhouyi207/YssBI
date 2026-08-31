use serde::Deserialize;
use yss_sci_runtime::api::time_series::serial_tests::{
    SerialTestWithLag, SerialTestsInput, SerialTestsOutput, compute_serial_tests,
};

const SIMPLE_RESIDUALS: &str =
    include_str!("fixtures/time_series/serial_tests/simple_residuals.json");
const WITH_EXOG_BG: &str = include_str!("fixtures/time_series/serial_tests/with_exog_bg.json");

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SerialTestsGoldenFixture {
    name: String,
    input: SerialTestsInput,
    expected: SerialTestsOutput,
    tolerance: Tolerance,
}

#[derive(Debug, Clone, Deserialize)]
struct Tolerance {
    absolute: f64,
}

#[test]
fn rust_serial_tests_match_golden_fixtures() {
    for fixture in fixtures() {
        let result = compute_serial_tests(fixture.input.clone())
            .unwrap_or_else(|error| panic!("{} rust serial tests failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &result,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }
}

fn fixtures() -> Vec<SerialTestsGoldenFixture> {
    vec![parse_fixture(SIMPLE_RESIDUALS), parse_fixture(WITH_EXOG_BG)]
}

fn parse_fixture(contents: &str) -> SerialTestsGoldenFixture {
    serde_json::from_str(contents).expect("valid serial tests golden fixture")
}

fn assert_output_close(
    name: &str,
    actual: &SerialTestsOutput,
    expected: &SerialTestsOutput,
    tolerance: f64,
) {
    match (&actual.bg, &expected.bg) {
        (Some(actual), Some(expected)) => {
            assert_lagged_close(name, "bg", actual, expected, tolerance);
        }
        (None, None) => {}
        _ => panic!(
            "{name} bg presence differs: actual={:?}, expected={:?}",
            actual.bg, expected.bg
        ),
    }

    match (&actual.q, &expected.q) {
        (Some(actual), Some(expected)) => {
            assert_lagged_close(name, "q", actual, expected, tolerance);
        }
        (None, None) => {}
        _ => panic!(
            "{name} q presence differs: actual={:?}, expected={:?}",
            actual.q, expected.q
        ),
    }

    assert_close(name, "dw.d", actual.dw.d, expected.dw.d, tolerance);
}

fn assert_lagged_close(
    name: &str,
    metric: &str,
    actual: &SerialTestWithLag,
    expected: &SerialTestWithLag,
    tolerance: f64,
) {
    assert_eq!(actual.lags, expected.lags, "{name} {metric}.lags");
    assert_close(
        name,
        &format!("{metric}.stat"),
        actual.stat,
        expected.stat,
        tolerance,
    );
    assert_close(
        name,
        &format!("{metric}.p_value"),
        actual.p_value,
        expected.p_value,
        tolerance,
    );
}

fn assert_close(name: &str, metric: &str, actual: f64, expected: f64, tolerance: f64) {
    let difference = (actual - expected).abs();
    assert!(
        difference <= tolerance,
        "{name} {metric} differs: actual={actual}, expected={expected}, diff={difference}, tolerance={tolerance}"
    );
}
