//! 解析 margins at() 规格（薄包装，复用假设检验 AST 管线）

use crate::application::hypothesis::parse_at_values as resolve_at_values;
use crate::error::AppError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub struct ParseAtRequest {
    pub param_names: Vec<String>,
    pub at_spec: String,
}

#[derive(Debug, Serialize)]
pub struct ParseAtResponse {
    pub values: std::collections::HashMap<String, f64>,
}

#[tauri::command]
pub fn parse_at_values(req: ParseAtRequest) -> Result<ParseAtResponse, AppError> {
    let values = resolve_at_values(&req.at_spec, &req.param_names)
        .map_err(|e| e.replace("解析假设失败", "解析 at() 失败"))?;
    Ok(ParseAtResponse { values })
}
