use serde::Deserialize;
use yssbi_lib::sci::api::bayes::{BayesModelDraft, validate_draft};

const INVALID_FIXTURES: &[(&str, &str)] = &[
    (
        "missing_dataset",
        include_str!("sci/fixtures/bayes/invalid/missing_dataset.json"),
    ),
    (
        "missing_sigma",
        include_str!("sci/fixtures/bayes/invalid/missing_sigma.json"),
    ),
    (
        "unbound_predictor",
        include_str!("sci/fixtures/bayes/invalid/unbound_predictor.json"),
    ),
    (
        "invalid_prior_args",
        include_str!("sci/fixtures/bayes/invalid/invalid_prior_args.json"),
    ),
];

#[derive(Debug, Deserialize)]
struct InvalidValidationFixture {
    name: String,
    draft: BayesModelDraft,
    expected: ExpectedValidation,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExpectedValidation {
    ok: bool,
    error_codes: Vec<String>,
}

#[test]
fn invalid_bayes_model_fixtures_report_expected_errors() {
    for (fixture_id, contents) in INVALID_FIXTURES {
        let fixture: InvalidValidationFixture = serde_json::from_str(contents)
            .unwrap_or_else(|error| panic!("invalid fixture {fixture_id}: {error}"));
        let report = validate_draft(&fixture.draft);
        assert_eq!(
            report.ok, fixture.expected.ok,
            "unexpected validation ok for {}",
            fixture.name
        );

        let actual_codes = report
            .errors
            .iter()
            .map(|issue| issue.code.as_str())
            .collect::<Vec<_>>();
        for expected_code in &fixture.expected.error_codes {
            assert!(
                actual_codes.contains(&expected_code.as_str()),
                "fixture {} expected error code {}, got {:?}",
                fixture.name,
                expected_code,
                actual_codes
            );
        }
    }
}

#[test]
fn invalid_bayes_model_error_codes_are_stable_and_machine_readable() {
    for (fixture_id, contents) in INVALID_FIXTURES {
        let fixture: InvalidValidationFixture = serde_json::from_str(contents)
            .unwrap_or_else(|error| panic!("invalid fixture {fixture_id}: {error}"));
        let report = validate_draft(&fixture.draft);
        assert!(
            !report.errors.is_empty(),
            "fixture {} should fail",
            fixture.name
        );
        for issue in &report.errors {
            assert!(
                issue
                    .code
                    .chars()
                    .all(|character| character.is_ascii_uppercase() || character == '_'),
                "validation code should be stable SCREAMING_SNAKE_CASE: {}",
                issue.code
            );
            assert!(issue.path.is_some(), "validation issue should include path");
            assert!(
                !issue.message.trim().is_empty(),
                "validation issue should include message"
            );
        }
    }
}
