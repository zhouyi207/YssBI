#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionNumericTolerance {
    pub absolute: f64,
    pub relative: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMissingValuePolicy {
    Listwise,
    Reject,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExecutionSettings {
    pub numeric_tolerance: ExecutionNumericTolerance,
    pub statistical_missing_values: ExecutionMissingValuePolicy,
}
