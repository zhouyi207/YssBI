//! 解析 margins at() 规格，复用假设检验 AST
//!
//! 输入如 "x1 = 0, x2 = 1.5"，输出 param_name -> value 映射。

use serde::{Deserialize, Serialize};

use crate::ast::{
    collect_param_order, linear_expand, parse_hypothesis_with_registry, ParamRegistry,
};

#[derive(Debug, Deserialize)]
pub struct ParseAtRequest {
    pub param_names: Vec<String>,
    pub at_spec: String,
}

#[derive(Debug, Serialize)]
pub struct ParseAtResponse {
    pub values: std::collections::HashMap<String, f64>,
}

/// 从 equality 约束解析 at() 值：param = value
fn run_parse_at_values(req: ParseAtRequest) -> Result<ParseAtResponse, String> {
    let trimmed = req.at_spec.trim();
    if trimmed.is_empty() {
        return Ok(ParseAtResponse {
            values: std::collections::HashMap::new(),
        });
    }

    let mut param_registry = ParamRegistry::new();
    let constraints = parse_hypothesis_with_registry(&req.at_spec, &mut param_registry)
        .map_err(|e| format!("解析 at() 失败: {}", e))?;

    if constraints.is_empty() {
        return Ok(ParseAtResponse {
            values: std::collections::HashMap::new(),
        });
    }

    let test_spec = linear_expand(&constraints).map_err(|e| format!("线性展开失败: {}", e))?;

    let (r, r_vec) = match &test_spec.hypothesis {
        crate::ast::HypothesisSpec::Linear { r, r_vec, .. } => (r.clone(), r_vec.clone()),
        crate::ast::HypothesisSpec::Nonlinear { .. } => {
            return Err("at() 仅支持线性等式约束 (param = value)".to_string());
        }
    };

    let param_order = collect_param_order(&constraints);
    let (r_ols, r_vec) =
        crate::ast::reorder_r_to_ols_columns(&r, &r_vec, &param_order, &param_registry, &req.param_names)
            .map_err(|e| format!("参数映射失败: {}", e))?;

    let mut values = std::collections::HashMap::new();
    for i in 0..r_ols.nrows() {
        let row = r_ols.row(i);
        let mut nonzero_col: Option<usize> = None;
        for j in 0..row.len() {
            if row[j].abs() > 1e-14 {
                if nonzero_col.is_some() {
                    return Err(format!(
                        "at() 约束 {} 涉及多个参数，请使用简单形式 param = value",
                        i + 1
                    ));
                }
                nonzero_col = Some(j);
            }
        }
        if let Some(j) = nonzero_col {
            let param_name = req.param_names[j].clone();
            let coeff = r_ols[[i, j]];
            let value = r_vec[i] / coeff;
            values.insert(param_name, value);
        }
    }

    Ok(ParseAtResponse { values })
}

#[tauri::command]
pub fn parse_at_values(req: ParseAtRequest) -> Result<ParseAtResponse, String> {
    run_parse_at_values(req)
}
