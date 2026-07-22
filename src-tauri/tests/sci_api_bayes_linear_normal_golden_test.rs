use polars::prelude::{Column, DataFrame};
use serde::Deserialize;
use yssbi_lib::sci::api::bayes::{
    BayesDataExchangeManifest, BayesExchangeColumn, BayesModelSpec, BinaryOp, DatasetSourceType,
    Expression, LikelihoodSpec, ParameterConstraint, PriorSpec,
};

const SIMPLE_LINEAR_NORMAL: &str = include_str!("sci/fixtures/bayes/linear_normal/simple.json");
const EXPONENTIAL_DECAY_NORMAL: &str =
    include_str!("sci/fixtures/bayes/nonlinear_normal/exponential_decay.json");
const SIMPLE_BERNOULLI_LOGIT: &str = include_str!("sci/fixtures/bayes/bernoulli_logit/simple.json");
const SIMPLE_POISSON_LOG: &str = include_str!("sci/fixtures/bayes/poisson_log/simple.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BayesGoldenFixture {
    name: String,
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
fn linear_normal_fixture_defines_stable_model_protocol() {
    let fixture = simple_linear_normal_fixture();
    assert_eq!(fixture.name, "simple linear normal");

    let spec = &fixture.model_spec;
    assert_eq!(spec.dataset.source_type, DatasetSourceType::Table);
    assert_eq!(
        spec.response.data_variables.get("y").map(String::as_str),
        Some("y")
    );
    assert_eq!(spec.data_variables.get("x").map(String::as_str), Some("x"));
    assert_eq!(spec.parameters.len(), 3);
    assert_eq!(spec.sampler.chains, 2);
    assert!(spec.sampler.save_samples);

    match &spec.likelihood {
        LikelihoodSpec::Normal { sigma, .. } => assert_eq!(sigma.parameter, "sigma"),
        other => panic!("expected normal likelihood, got {other:?}"),
    }

    let parameter_names = spec
        .parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(parameter_names, ["a", "b", "sigma"]);
    assert!(matches!(
        spec.parameters[0].constraint,
        ParameterConstraint::Real
    ));
    assert!(matches!(
        spec.parameters[0].prior,
        PriorSpec::Normal([0.0, 10.0])
    ));
    assert!(matches!(
        spec.parameters[2].prior,
        PriorSpec::Exponential([1.0])
    ));

    assert_eq!(fixture.golden.parameters, ["a", "b", "sigma"]);
    assert!(fixture.golden.max_rhat > 1.0);
    assert!(fixture.golden.requires_samples);
    assert!(fixture.golden.requires_posterior_predictive);
    assert_posterior_mean_expectation(&fixture, "a", 2.0, 1.0);
}

#[test]
fn linear_normal_fixture_materializes_input_dataframe() {
    let fixture = simple_linear_normal_fixture();
    let dataframe = fixture_input_table(&fixture);
    assert_eq!(dataframe.height(), 6);
    assert_eq!(dataframe.get_column_names(), ["x", "y"]);
}

#[test]
fn nonlinear_normal_fixture_defines_generic_expression_protocol() {
    let fixture = exponential_decay_normal_fixture();
    assert_eq!(fixture.name, "exponential decay normal");

    let spec = &fixture.model_spec;
    assert_eq!(
        spec.response.data_variables.get("y").map(String::as_str),
        Some("y")
    );
    assert_eq!(spec.data_variables.get("x").map(String::as_str), Some("x"));
    assert_eq!(spec.parameters.len(), 4);
    assert_eq!(fixture.golden.parameters, ["a", "b", "c", "sigma"]);
    assert!(fixture.golden.requires_samples);
    assert!(fixture.golden.requires_posterior_predictive);

    match &spec.predictor {
        Expression::Binary { op, left, right } => {
            assert_eq!(*op, BinaryOp::Add);
            assert!(matches!(left.as_ref(), Expression::Binary { .. }));
            assert!(matches!(right.as_ref(), Expression::Parameter { name } if name == "c"));
        }
        other => panic!("expected binary predictor, got {other:?}"),
    }

    match &spec.likelihood {
        LikelihoodSpec::Normal { sigma, .. } => assert_eq!(sigma.parameter, "sigma"),
        other => panic!("expected normal likelihood, got {other:?}"),
    }
    assert!(matches!(
        spec.parameters[1].constraint,
        ParameterConstraint::Positive
    ));
    assert!(matches!(
        spec.parameters[1].prior,
        PriorSpec::Exponential([1.0])
    ));

    let dataframe = fixture_input_table(&fixture);
    assert_eq!(dataframe.height(), 8);
    assert_eq!(dataframe.get_column_names(), ["x", "y"]);
}

#[test]
fn bernoulli_logit_fixture_defines_model_family_protocol() {
    let fixture = simple_bernoulli_logit_fixture();
    assert_eq!(fixture.name, "simple bernoulli logit");
    let spec = &fixture.model_spec;
    assert_eq!(spec.parameters.len(), 2);
    assert_eq!(fixture.golden.parameters, ["a", "b"]);
    assert!(matches!(
        spec.likelihood,
        LikelihoodSpec::BernoulliLogit { .. }
    ));
    assert_eq!(spec.data_variables.get("x").map(String::as_str), Some("x"));
    assert!(spec.sampler.save_samples);
    assert_eq!(fixture_input_table(&fixture).height(), 8);
}

#[test]
fn poisson_log_fixture_defines_model_family_protocol() {
    let fixture = simple_poisson_log_fixture();
    assert_eq!(fixture.name, "simple poisson log");
    let spec = &fixture.model_spec;
    assert_eq!(spec.parameters.len(), 2);
    assert_eq!(fixture.golden.parameters, ["a", "b"]);
    assert!(matches!(spec.likelihood, LikelihoodSpec::PoissonLog { .. }));
    assert_eq!(spec.data_variables.get("x").map(String::as_str), Some("x"));
    assert!(spec.sampler.save_samples);
    assert_eq!(fixture_input_table(&fixture).height(), 8);
}

#[test]
fn linear_normal_fixture_exchange_manifest_is_stable() {
    let fixture = simple_linear_normal_fixture();
    let dataframe = fixture_input_table(&fixture);
    let manifest = BayesDataExchangeManifest::new(
        "bayes-fixture",
        "input.arrow",
        "model_spec.json",
        "inference_config.json",
        "predictor_kernel.jl",
        "likelihood_kernel.jl",
        vec!["x".to_string()],
        "output.arrow",
        "metadata.json",
        dataframe.height(),
        dataframe
            .get_column_names()
            .into_iter()
            .map(|name| BayesExchangeColumn {
                name: name.to_string(),
            })
            .collect(),
    );

    let value = serde_json::to_value(&manifest).expect("manifest JSON");
    assert_eq!(value["version"], 3);
    assert_eq!(value["taskId"], "bayes-fixture");
    assert_eq!(value["predictorKernelPath"], "predictor_kernel.jl");
    assert_eq!(value["likelihoodKernelPath"], "likelihood_kernel.jl");
    assert_eq!(value["predictorColumns"], serde_json::json!(["x"]));
    assert_eq!(value["inputRows"], 6);
    assert_eq!(value["inputColumns"][0]["name"], "x");
    assert_eq!(value["inputColumns"][1]["name"], "y");
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

fn assert_posterior_mean_expectation(
    fixture: &BayesGoldenFixture,
    parameter: &str,
    expected: f64,
    tolerance: f64,
) {
    let actual = fixture
        .golden
        .posterior_mean
        .get(parameter)
        .expect("posterior mean expectation");
    assert_eq!(actual.expected, expected);
    assert_eq!(actual.tolerance, tolerance);
}
