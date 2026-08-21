use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectExecutionErrorKind {
    Internal,
    StaleProjectLifecycle,
    RecoveryRequired,
    InvalidDemand,
    Run,
    InternalCompilation,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectExecutionError {
    kind: ProjectExecutionErrorKind,
    diagnostic: Box<str>,
    run_error: Option<crate::node_system::runtime::RunError>,
    internal_compilation_failure: Option<crate::node_system::compiler::InternalCompilationFailure>,
}

impl ProjectExecutionError {
    pub fn internal(diagnostic: impl Into<Box<str>>) -> Self {
        Self {
            kind: ProjectExecutionErrorKind::Internal,
            diagnostic: diagnostic.into(),
            run_error: None,
            internal_compilation_failure: None,
        }
    }

    pub fn stale_project_lifecycle(diagnostic: impl Into<Box<str>>) -> Self {
        Self {
            kind: ProjectExecutionErrorKind::StaleProjectLifecycle,
            diagnostic: diagnostic.into(),
            run_error: None,
            internal_compilation_failure: None,
        }
    }

    pub fn recovery_required(diagnostic: impl Into<Box<str>>) -> Self {
        Self {
            kind: ProjectExecutionErrorKind::RecoveryRequired,
            diagnostic: diagnostic.into(),
            run_error: None,
            internal_compilation_failure: None,
        }
    }

    pub fn invalid_demand(diagnostic: impl Into<Box<str>>) -> Self {
        Self {
            kind: ProjectExecutionErrorKind::InvalidDemand,
            diagnostic: diagnostic.into(),
            run_error: None,
            internal_compilation_failure: None,
        }
    }

    pub const fn kind(&self) -> ProjectExecutionErrorKind {
        self.kind
    }

    pub fn internal_compilation(
        failure: crate::node_system::compiler::InternalCompilationFailure,
    ) -> Self {
        let node_context = failure
            .node_id
            .map(|node_id| format!(" (node {node_id})"))
            .unwrap_or_default();
        Self {
            kind: ProjectExecutionErrorKind::InternalCompilation,
            diagnostic: format!(
                "internal compilation failure at {:?}: {}{node_context}",
                failure.stage, failure.code
            )
            .into(),
            run_error: None,
            internal_compilation_failure: Some(failure),
        }
    }

    pub fn run_error(&self) -> Option<&crate::node_system::runtime::RunError> {
        self.run_error.as_ref()
    }

    pub fn internal_compilation_failure(
        &self,
    ) -> Option<&crate::node_system::compiler::InternalCompilationFailure> {
        self.internal_compilation_failure.as_ref()
    }

    pub fn contains(&self, pattern: &str) -> bool {
        self.diagnostic.contains(pattern)
    }
}

impl PartialEq<&str> for ProjectExecutionError {
    fn eq(&self, other: &&str) -> bool {
        self.diagnostic.as_ref() == *other
    }
}

impl From<crate::node_system::runtime::RunError> for ProjectExecutionError {
    fn from(error: crate::node_system::runtime::RunError) -> Self {
        let message = match crate::node_system::runtime::RunErrorOutcome::from(&error) {
            crate::node_system::runtime::RunErrorOutcome::Ordinary { code } => {
                code.public_message()
            }
            crate::node_system::runtime::RunErrorOutcome::DeadlineExceeded { .. } => {
                "run deadline was exceeded"
            }
        };
        Self {
            kind: ProjectExecutionErrorKind::Run,
            diagnostic: message.into(),
            run_error: Some(error),
            internal_compilation_failure: None,
        }
    }
}

impl From<String> for ProjectExecutionError {
    fn from(diagnostic: String) -> Self {
        Self::internal(diagnostic)
    }
}

impl From<&str> for ProjectExecutionError {
    fn from(diagnostic: &str) -> Self {
        Self::internal(diagnostic)
    }
}

impl From<ProjectFilesystemError> for ProjectExecutionError {
    fn from(error: ProjectFilesystemError) -> Self {
        if error.recovery_required() {
            Self::recovery_required(error.to_string())
        } else if matches!(error, ProjectFilesystemError::StaleProjectLifecycle { .. }) {
            Self::stale_project_lifecycle(error.to_string())
        } else {
            Self::internal(error.to_string())
        }
    }
}

impl std::fmt::Display for ProjectExecutionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.diagnostic)
    }
}

impl std::error::Error for ProjectExecutionError {}
