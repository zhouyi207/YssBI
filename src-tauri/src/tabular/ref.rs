use crate::variable::VariableId;

pub const VAR_PREFIX: &str = "var:";

/// 变量 tabular 的稳定 handle（`var:{variable_uuid}`）。
pub fn variable_handle(id: &VariableId) -> String {
    format!("{VAR_PREFIX}{id}")
}

pub fn variable_handle_str(variable_id: &str) -> String {
    format!("{VAR_PREFIX}{variable_id}")
}

pub fn is_variable_handle(id: &str) -> bool {
    id.starts_with(VAR_PREFIX)
}

pub fn variable_id_from_handle(id: &str) -> Option<&str> {
    id.strip_prefix(VAR_PREFIX)
}
