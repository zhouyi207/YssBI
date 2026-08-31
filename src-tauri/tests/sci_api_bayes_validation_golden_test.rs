use serde::Deserialize;
use yss_bayes_model::{BayesModelDraft, validate_draft};

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
    errors: Vec<ExpectedValidationIssue>,
}

#[derive(Debug, Deserialize)]
struct ExpectedValidationIssue {
    code: String,
    path: String,
}

#[test]
fn invalid_bayes_model_fixtures_report_expected_errors() {
    for (fixture_id, contents) in INVALID_FIXTURES {
        let fixture: InvalidValidationFixture = serde_json::from_str(contents)
            .unwrap_or_else(|error| panic!("invalid fixture {fixture_id}: {error}"));
        let report = validate_draft(&fixture.draft);
        assert_eq!(
            report.is_ok(),
            fixture.expected.ok,
            "unexpected validation ok for {}",
            fixture.name
        );

        for expected in &fixture.expected.errors {
            assert!(
                report
                    .errors()
                    .iter()
                    .any(|issue| { issue.code == expected.code && issue.path == expected.path }),
                "fixture {} expected error {} at {}, got {:?}",
                fixture.name,
                expected.code,
                expected.path,
                report.errors()
            );
        }
    }
}
