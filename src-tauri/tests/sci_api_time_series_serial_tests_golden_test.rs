use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use uuid::Uuid;
use yssbi_lib::julia::worker::JuliaWorkerManager;
use yssbi_lib::sci::api::time_series::serial_tests::{
    SerialTestsInput, SerialTestsOutput, compute_serial_tests,
};
use yssbi_lib::sci::engine::{SciContext, SciEngine};

const SIMPLE_RESIDUALS: &str =
    include_str!("sci/fixtures/time_series/serial_tests/simple_residuals.json");
const WITH_EXOG_BG: &str = include_str!("sci/fixtures/time_series/serial_tests/with_exog_bg.json");

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
        let result = compute_serial_tests(&SciContext::rust(), fixture.input.clone())
            .unwrap_or_else(|error| panic!("{} rust serial tests failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &result,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }
}

#[test]
fn julia_serial_tests_match_golden_fixtures_when_enabled() {
    if std::env::var_os("YSSBI_RUN_JULIA_TESTS").is_none() {
        eprintln!("skipped: set YSSBI_RUN_JULIA_TESTS=1 to run Julia worker golden tests");
        return;
    }

    let app_data_dir = temp_app_data_dir();
    let worker = JuliaWorkerManager::new();
    worker.prepare(&app_data_dir).expect("prepare Julia worker");

    let context = SciContext::with_julia(&app_data_dir, &worker, SciEngine::Julia);

    for fixture in fixtures() {
        let result = compute_serial_tests(&context, fixture.input.clone())
            .unwrap_or_else(|error| panic!("{} julia serial tests failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &result,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }

    let _ = fs::remove_dir_all(app_data_dir);
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
    actual: &yssbi_lib::sci::api::time_series::serial_tests::SerialTestWithLag,
    expected: &yssbi_lib::sci::api::time_series::serial_tests::SerialTestWithLag,
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

fn temp_app_data_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("yssbi-julia-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp app data dir");
    path
}
