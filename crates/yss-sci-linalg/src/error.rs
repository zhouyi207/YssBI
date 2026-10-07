#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinalgError {
    NotSquare,
    NotPositiveDefinite,
    Singular,
    DecompositionFailed,
}

impl std::fmt::Display for LinalgError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotSquare => "matrix must be square",
            Self::NotPositiveDefinite => "matrix is not positive definite",
            Self::Singular => "matrix is singular",
            Self::DecompositionFailed => "matrix decomposition failed",
        })
    }
}

impl std::error::Error for LinalgError {}
