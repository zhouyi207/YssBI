#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseAccess {
    /// UI / Inspector / Preview
    Preview,

    /// Graph execution / runtime
    Execution,
}
