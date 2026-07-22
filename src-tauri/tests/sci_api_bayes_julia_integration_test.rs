use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use polars::prelude::{Column, DataFrame};
use serde::Deserialize;
use uuid::Uuid;
use yssbi_lib::julia::worker::JuliaWorkerManager;
use yssbi_lib::sci::api::bayes::{
    BayesBackend, BayesBackendRequest, BayesModelSpec, ResultArtifactKind, TaskProgress,
};
use yssbi_lib::sci::backends::julia::bayes::JuliaBayesBackend;

const SIMPLE_LINEAR_NORMAL: &str = include_str!("sci/fixtures/bayes/linear_normal/simple.json");
const EXPONENTIAL_DECAY_NORMAL: &str =
    include_str!("sci/fixtures/bayes/nonlinear_normal/exponential_decay.json");
const SIMPLE_BERNOULLI_LOGIT: &str = include_str!("sci/fixtures/bayes/bernoulli_logit/simple.json");
const SIMPLE_POISSON_LOG: &str = include_str!("sci/fixtures/bayes/poisson_log/simple.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BayesGoldenFixture {
    data: FixtureData,
    model_spec: BayesModelSpec,
    golden: GoldenExpectations,
}

#[derive(Debug, Deserialize)]
struct FixtureData {
    columns: Vec<FixtureColumn>,
}

#[derive(Debug, Deserialize)]
struct FixtureColumn {
    name: String,
    values: Vec<f64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct GoldenExpectations {
    parameters: Vec<String>,
    posterior_mean: std::collections::BTreeMap<String, PosteriorMeanExpectation>,
    max_rhat: f64,
    requires_samples: bool,
    requires_posterior_predictive: bool,
}

#[derive(Debug, Deserialize)]
struct PosteriorMeanExpectation {
    expected: f64,
    tolerance: f64,
}

#[test]
fn julia_bayes_generated_linear_runs_when_enabled() {
    run_julia_fixture_when_enabled(simple_linear_normal_fixture(), "julia-bayes-linear-test");
}

#[test]
fn julia_bayes_generic_normal_runs_when_enabled() {
    run_julia_fixture_when_enabled(
        exponential_decay_normal_fixture(),
        "julia-bayes-nonlinear-test",
    );
}

#[test]
fn julia_bayes_bernoulli_logit_runs_when_enabled() {
    run_julia_fixture_when_enabled(
        simple_bernoulli_logit_fixture(),
        "julia-bayes-bernoulli-logit-test",
    );
}

#[test]
fn julia_bayes_poisson_log_runs_when_enabled() {
    run_julia_fixture_when_enabled(simple_poisson_log_fixture(), "julia-bayes-poisson-log-test");
}

fn run_julia_fixture_when_enabled(fixture: BayesGoldenFixture, task_id: &str) {
    if std::env::var_os("YSSBI_RUN_JULIA_BAYES_TESTS").is_none() {
        eprintln!(
            "skipped: set YSSBI_RUN_JULIA_BAYES_TESTS=1 to run Julia Bayesian integration tests"
        );
        return;
    }

    let app_data_dir = temp_app_data_dir();
    let worker = JuliaWorkerManager::new();
    worker.warm_up(&app_data_dir).expect("warm up Julia worker");

    let backend = JuliaBayesBackend::new(&app_data_dir, worker);
    let expected_total = fixture.model_spec.sampler.chains
        * (fixture.model_spec.sampler.warmup + fixture.model_spec.sampler.samples);
    let progress_updates = Arc::new(Mutex::new(Vec::<TaskProgress>::new()));
    let progress_target = progress_updates.clone();
    let result = backend
        .fit(BayesBackendRequest::with_progress(
            task_id,
            fixture.model_spec.clone(),
            Some(fixture_input_table(&fixture)),
            Some(Arc::new(move |progress| {
                progress_target.lock().unwrap().push(progress);
            })),
        ))
        .expect("Julia Bayesian backend result");

    let progress_updates = progress_updates.lock().unwrap();
    for expected_stage in [
        "loading_data",
        "loading_kernels",
        "preparing_kernels",
        "building_model",
        "initializing_nuts",
        "warmup",
    ] {
        assert!(
            progress_updates
                .iter()
                .any(|progress| progress.stage == expected_stage),
            "missing progress stage {expected_stage}",
        );
    }
    assert!(
        progress_updates
            .iter()
            .any(|progress| progress.stage == "sampling")
    );
    let final_sampling_progress = progress_updates
        .iter()
        .rev()
        .find(|progress| progress.completed.is_some())
        .expect("numeric Julia sampling progress");
    assert_eq!(final_sampling_progress.completed, Some(expected_total));
    assert_eq!(final_sampling_progress.total, Some(expected_total));

    assert_eq!(result.summaries.len(), fixture.golden.parameters.len());
    for parameter in &fixture.golden.parameters {
        assert!(
            result
                .summaries
                .iter()
                .any(|summary| summary.parameter == *parameter),
            "missing summary for {parameter}"
        );
    }
    assert!(
        result
            .summaries
            .iter()
            .all(|summary| summary.rhat.is_some())
    );
    assert!(
        result
            .summaries
            .iter()
            .all(|summary| summary.ess_bulk.is_some())
    );
    assert!(
        result
            .summaries
            .iter()
            .all(|summary| summary.ess_tail.is_some())
    );
    if fixture.golden.requires_samples {
        assert!(
            result
                .artifact_manifest
                .artifacts
                .iter()
                .any(|artifact| { artifact.kind == ResultArtifactKind::PosteriorSamples })
        );
    }
    if fixture.golden.requires_posterior_predictive {
        assert!(
            result
                .artifact_manifest
                .artifacts
                .iter()
                .any(|artifact| { artifact.kind == ResultArtifactKind::PosteriorPredictive })
        );
    }
    for summary in &result.summaries {
        if let Some(expectation) = fixture.golden.posterior_mean.get(&summary.parameter) {
            assert!(
                (summary.mean - expectation.expected).abs() <= expectation.tolerance,
                "posterior mean for {} was {}, expected {} ± {}",
                summary.parameter,
                summary.mean,
                expectation.expected,
                expectation.tolerance
            );
        }
        if let Some(rhat) = summary.rhat {
            assert!(
                rhat <= fixture.golden.max_rhat,
                "R-hat for {} was {}, expected <= {}",
                summary.parameter,
                rhat,
                fixture.golden.max_rhat
            );
        }
    }
    assert!(result.diagnostics.warnings.iter().all(|warning| {
        !warning.code.ends_with("_READY") && !warning.code.contains("TURING_GENERIC")
    }));

    let _ = fs::remove_dir_all(app_data_dir);
}

fn simple_linear_normal_fixture() -> BayesGoldenFixture {
    serde_json::from_str(SIMPLE_LINEAR_NORMAL).expect("valid linear normal fixture")
}

fn exponential_decay_normal_fixture() -> BayesGoldenFixture {
    serde_json::from_str(EXPONENTIAL_DECAY_NORMAL).expect("valid nonlinear normal fixture")
}

fn simple_bernoulli_logit_fixture() -> BayesGoldenFixture {
    serde_json::from_str(SIMPLE_BERNOULLI_LOGIT).expect("valid bernoulli logit fixture")
}

fn simple_poisson_log_fixture() -> BayesGoldenFixture {
    serde_json::from_str(SIMPLE_POISSON_LOG).expect("valid poisson log fixture")
}

fn fixture_input_table(fixture: &BayesGoldenFixture) -> DataFrame {
    let columns = fixture
        .data
        .columns
        .iter()
        .map(|column| Column::new(column.name.clone().into(), column.values.as_slice()))
        .collect::<Vec<_>>();
    DataFrame::new(fixture.data.columns[0].values.len(), columns).expect("fixture dataframe")
}

fn temp_app_data_dir() -> PathBuf {
    let path = std::env::temp_dir().join(format!("yssbi-julia-bayes-test-{}", Uuid::new_v4()));
    fs::create_dir_all(&path).expect("create temp app data dir");
    path
}
