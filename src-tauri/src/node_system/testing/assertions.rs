use crate::node_system::compiler::CompileResult;
use crate::node_system::plan::ExecutionPlan;
use crate::node_system::protocol::Value;
use crate::node_system::runtime::{RunError, RunResult, RuntimeValue};

pub fn compile_assertions(result: CompileResult) -> CompileAssertions {
    CompileAssertions { result }
}

pub struct CompileAssertions {
    result: CompileResult,
}

impl CompileAssertions {
    #[track_caller]
    pub fn has_plan(self) -> Self {
        assert!(
            self.result.plan.is_some(),
            "expected a plan, outcome was {:?}, diagnostics were: {:#?}",
            self.result.outcome,
            self.result.analysis.diagnostics
        );
        self
    }

    #[track_caller]
    pub fn has_no_plan(self) -> Self {
        assert!(
            self.result.plan.is_none(),
            "expected compilation without a plan, outcome was {:?}, plan was: {:#?}",
            self.result.outcome,
            self.result.plan
        );
        self
    }

    #[track_caller]
    pub fn has_no_diagnostics(self) -> Self {
        assert!(
            self.result.analysis.diagnostics.is_empty(),
            "unexpected diagnostics: {:#?}",
            self.result.analysis.diagnostics
        );
        self
    }

    #[track_caller]
    pub fn has_diagnostic(self, code: &str) -> Self {
        assert!(
            self.result
                .analysis
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code.as_str() == code),
            "missing diagnostic '{code}', got: {:#?}",
            self.result.analysis.diagnostics
        );
        self
    }

    pub fn result(&self) -> &CompileResult {
        &self.result
    }

    #[track_caller]
    pub fn into_plan(self) -> ExecutionPlan {
        self.result.plan.unwrap_or_else(|| {
            panic!(
                "expected a plan, outcome was {:?}, diagnostics were: {:#?}",
                self.result.outcome, self.result.analysis.diagnostics
            )
        })
    }

    pub fn into_result(self) -> CompileResult {
        self.result
    }
}

pub fn run_assertions(result: Result<RunResult, RunError>) -> RunAssertions {
    RunAssertions { result }
}

pub struct RunAssertions {
    result: Result<RunResult, RunError>,
}

impl RunAssertions {
    #[track_caller]
    pub fn succeeds(self) -> Self {
        if let Err(error) = &self.result {
            panic!("expected run success, got: {error:#?}");
        }
        self
    }

    #[track_caller]
    pub fn fails(self) -> Self {
        if let Ok(result) = &self.result {
            panic!("expected run failure, got: {result:#?}");
        }
        self
    }

    #[track_caller]
    pub fn is_cancelled(self) -> Self {
        assert_eq!(self.result.as_ref().unwrap_err(), &RunError::Cancelled);
        self
    }

    #[track_caller]
    pub fn has_value(self, name: &str, expected: &Value) -> Self {
        let result = self
            .result
            .as_ref()
            .unwrap_or_else(|error| panic!("expected run success, got: {error:#?}"));
        match result.values.get(name) {
            Some(RuntimeValue::Scalar(actual)) => {
                assert_eq!(actual, expected, "unexpected result value for '{name}'");
            }
            Some(actual) => {
                panic!("expected scalar result value for '{name}', got: {actual:#?}");
            }
            None => panic!("missing result value '{name}'"),
        }
        self
    }

    #[track_caller]
    pub fn error_matches(self, predicate: impl FnOnce(&RunError) -> bool) -> Self {
        let error = self
            .result
            .as_ref()
            .err()
            .unwrap_or_else(|| panic!("expected run failure, got: {:#?}", self.result));
        assert!(predicate(error), "unexpected run error: {error:#?}");
        self
    }

    pub fn into_result(self) -> Result<RunResult, RunError> {
        self.result
    }
}
