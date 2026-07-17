use std::fs;
use std::path::PathBuf;

use serde::Deserialize;
use uuid::Uuid;
use yssbi_lib::julia::worker::JuliaWorkerManager;
use yssbi_lib::sci::api::time_series::acf_pacf::{AcfPacfInput, AcfPacfOutput, compute_acf_pacf};
use yssbi_lib::sci::engine::{SciContext, SciEngine};

const SIMPLE_EXPONENTIAL: &str =
    include_str!("sci/fixtures/time_series/acf_pacf/simple_exponential.json");

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
        let result = compute_acf_pacf(&SciContext::rust(), fixture.input.clone())
            .unwrap_or_else(|error| panic!("{} rust acf/pacf failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &result,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }
}

#[test]
fn julia_acf_pacf_matches_golden_fixtures_when_enabled() {
    if std::env::var_os("YSSBI_RUN_JULIA_TESTS").is_none() {
        eprintln!("skipped: set YSSBI_RUN_JULIA_TESTS=1 to run Julia worker golden tests");
        return;
    }

    let app_data_dir = temp_app_data_dir();
    let worker = JuliaWorkerManager::new();
    worker.prepare(&app_data_dir).expect("prepare Julia worker");

    let context = SciContext::with_julia(&app_data_dir, &worker, SciEngine::Julia);

    for fixture in fixtures() {
        let response = compute_acf_pacf(&context, fixture.input.clone())
            .unwrap_or_else(|error| panic!("{} julia acf/pacf failed: {error}", fixture.name));

        assert_output_close(
            &fixture.name,
            &response,
            &fixture.expected,
            fixture.tolerance.absolute,
        );
    }

    let _ = fs::remove_dir_all(app_data_dir);
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

fn temp_app_data_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("yssbi-julia-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp app data dir");
    path
}
