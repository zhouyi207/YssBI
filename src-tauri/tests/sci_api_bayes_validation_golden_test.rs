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
            report.ok, fixture.expected.ok,
            "unexpected validation ok for {}",
            fixture.name
        );

        for expected in &fixture.expected.errors {
            assert!(
                report
                    .errors
                    .iter()
                    .any(|issue| { issue.code == expected.code && issue.path == expected.path }),
                "fixture {} expected error {} at {}, got {:?}",
                fixture.name,
                expected.code,
                expected.path,
                report.errors
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
        for issue in report.errors.iter().chain(&report.warnings) {
            assert!(
                issue
                    .code
                    .starts_with(|character: char| character.is_ascii_lowercase())
                    && issue.code.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '_'
                    })
                    && !issue.code.ends_with('_')
                    && !issue.code.contains("__"),
                "validation code should be stable lower_snake_case: {}",
                issue.code
            );
            assert!(
                !issue.path.is_empty(),
                "validation issue should include path"
            );
            let wire = serde_json::to_value(issue).expect("serialize validation issue");
            let fields = wire.as_object().expect("validation issue object");
            assert_eq!(fields.len(), 3);
            assert!(fields.contains_key("code"));
            assert!(fields.contains_key("severity"));
            assert!(fields.contains_key("path"));
            assert!(wire.get("message").is_none());
            assert!(wire.get("hint").is_none());
            assert!(wire.get("details").is_none());
        }
    }
}
