use polars::prelude::{Column, DataFrame};
use serde::Deserialize;
use yssbi_lib::sci::api::bayes::{
    BayesModelSpec, Expression, MathFunction, validate_bayes_input_table,
};

const SIMPLE_LINEAR_NORMAL: &str = include_str!("sci/fixtures/bayes/linear_normal/simple.json");
const SIMPLE_BERNOULLI_LOGIT: &str = include_str!("sci/fixtures/bayes/bernoulli_logit/simple.json");
const SIMPLE_POISSON_LOG: &str = include_str!("sci/fixtures/bayes/poisson_log/simple.json");

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BayesFixture {
    model_spec: BayesModelSpec,
}

#[test]
fn valid_input_fixtures_pass_runtime_validation() {
    let normal = fixture(SIMPLE_LINEAR_NORMAL);
    validate_bayes_input_table(&normal.model_spec, &linear_table()).expect("valid Normal table");

    let bernoulli = fixture(SIMPLE_BERNOULLI_LOGIT);
    validate_bayes_input_table(
        &bernoulli.model_spec,
        &bernoulli_table(&[0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0]),
    )
    .expect("valid BernoulliLogit table");

    let poisson = fixture(SIMPLE_POISSON_LOG);
    validate_bayes_input_table(
        &poisson.model_spec,
        &poisson_table(&[1.0, 1.0, 2.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
    )
    .expect("valid PoissonLog table");
}

#[test]
fn normal_response_rejects_non_finite_values() {
    let fixture = fixture(SIMPLE_LINEAR_NORMAL);
    let table = DataFrame::new(
        3,
        vec![
            Column::new("x".into(), &[1.0, 2.0, 3.0]),
            Column::new("y".into(), &[3.0, f64::NAN, 7.0]),
        ],
    )
    .expect("test dataframe");

    let error = validate_bayes_input_table(&fixture.model_spec, &table).expect_err("invalid input");
    assert_eq!(error.code, "bayes_input_response_non_finite");
    assert_eq!(error.column.as_deref(), Some("y"));
    assert_eq!(error.row, Some(1));
}

#[test]
fn transformed_normal_response_rejects_ln_domain_errors() {
    let mut fixture = fixture(SIMPLE_LINEAR_NORMAL);
    fixture.model_spec.response.expression = Expression::Call {
        function: MathFunction::Ln,
        args: vec![Expression::DataVariable { name: "y".into() }],
    };
    let table = DataFrame::new(
        3,
        vec![
            Column::new("x".into(), &[1.0, 2.0, 3.0]),
            Column::new("y".into(), &[3.0, 0.0, 7.0]),
        ],
    )
    .expect("test dataframe");

    let error =
        validate_bayes_input_table(&fixture.model_spec, &table).expect_err("invalid ln domain");
    assert_eq!(error.code, "bayes_input_response_ln_domain");
    assert_eq!(error.column.as_deref(), Some("y"));
    assert_eq!(error.row, Some(1));
}

#[test]
fn predictor_rejects_non_finite_values() {
    let fixture = fixture(SIMPLE_LINEAR_NORMAL);
    let table = DataFrame::new(
        3,
        vec![
            Column::new("x".into(), &[1.0, f64::INFINITY, 3.0]),
            Column::new("y".into(), &[3.0, 5.0, 7.0]),
        ],
    )
    .expect("test dataframe");

    let error = validate_bayes_input_table(&fixture.model_spec, &table).expect_err("invalid input");
    assert_eq!(error.code, "bayes_input_predictor_non_finite");
    assert_eq!(error.column.as_deref(), Some("x"));
    assert_eq!(error.row, Some(1));
}

#[test]
fn bernoulli_response_rejects_values_outside_zero_one() {
    let fixture = fixture(SIMPLE_BERNOULLI_LOGIT);
    let error = validate_bayes_input_table(&fixture.model_spec, &bernoulli_table(&[0.0, 1.0, 2.0]))
        .expect_err("invalid Bernoulli response");
    assert_eq!(error.code, "bayes_input_bernoulli_response_invalid");
    assert_eq!(error.column.as_deref(), Some("y"));
    assert_eq!(error.row, Some(2));
}

#[test]
fn poisson_response_rejects_negative_counts() {
    let fixture = fixture(SIMPLE_POISSON_LOG);
    let error = validate_bayes_input_table(&fixture.model_spec, &poisson_table(&[1.0, -1.0, 2.0]))
        .expect_err("invalid Poisson response");
    assert_eq!(error.code, "bayes_input_poisson_response_negative");
    assert_eq!(error.column.as_deref(), Some("y"));
    assert_eq!(error.row, Some(1));
}

#[test]
fn poisson_response_rejects_fractional_counts() {
    let fixture = fixture(SIMPLE_POISSON_LOG);
    let error = validate_bayes_input_table(&fixture.model_spec, &poisson_table(&[1.0, 1.5, 2.0]))
        .expect_err("invalid Poisson response");
    assert_eq!(error.code, "bayes_input_poisson_response_not_integer");
    assert_eq!(error.column.as_deref(), Some("y"));
    assert_eq!(error.row, Some(1));
}

#[test]
fn missing_required_columns_are_reported_before_julia() {
    let fixture = fixture(SIMPLE_LINEAR_NORMAL);
    let table =
        DataFrame::new(3, vec![Column::new("y".into(), &[3.0, 5.0, 7.0])]).expect("test dataframe");

    let error =
        validate_bayes_input_table(&fixture.model_spec, &table).expect_err("missing column");
    assert_eq!(error.code, "bayes_input_column_missing");
    assert_eq!(error.column.as_deref(), Some("x"));
}

fn fixture(contents: &str) -> BayesFixture {
    serde_json::from_str(contents).expect("valid fixture")
}

fn linear_table() -> DataFrame {
    DataFrame::new(
        6,
        vec![
            Column::new("x".into(), &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            Column::new("y".into(), &[3.1, 5.0, 7.2, 8.9, 11.1, 13.0]),
        ],
    )
    .expect("linear table")
}

fn bernoulli_table(y: &[f64]) -> DataFrame {
    let x = (0..y.len()).map(|index| index as f64).collect::<Vec<_>>();
    DataFrame::new(
        y.len(),
        vec![
            Column::new("x".into(), x.as_slice()),
            Column::new("y".into(), y),
        ],
    )
    .expect("bernoulli table")
}

fn poisson_table(y: &[f64]) -> DataFrame {
    let x = (0..y.len()).map(|index| index as f64).collect::<Vec<_>>();
    DataFrame::new(
        y.len(),
        vec![
            Column::new("x".into(), x.as_slice()),
            Column::new("y".into(), y),
        ],
    )
    .expect("poisson table")
}
