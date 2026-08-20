use crate::variable::VariableId;

const VAR_PREFIX: &str = "var:";

/// 变量 tabular 的稳定 handle（`var:{variable_uuid}`）。
pub fn variable_handle(id: &VariableId) -> String {
    format!("{VAR_PREFIX}{id}")
}

pub(super) fn is_variable_handle(id: &str) -> bool {
    id.starts_with(VAR_PREFIX)
}
