//! 假设检验 Tauri 命令
//!
//! 从自然语言约束解析 → 参数映射 → 按 TestMethod 分发（TTest / Wald）→ 返回 JSON。

use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use yss_sci::stats::{t_test, wald_test, Alternative as SciAlternative};

use crate::ast::{
    collect_param_order, linear_expand, parse_hypothesis_with_registry, reorder_r_to_ols_columns,
    HypothesisExpr, HypothesisSpec, LinearConstraintKind, ParamRegistry, TestMethod,
};

/// 假设检验请求
#[derive(Debug, Deserialize)]
pub struct HypothesisTestRequest {
    /// OLS 参数估计 (k)
    pub betas: Vec<f64>,
    /// 参数协方差矩阵 (k×k)，行优先
    pub cov_beta: Vec<Vec<f64>>,
    /// 残差自由度
    pub df_residual: usize,
    /// 参数名，与 OLS exog 列序一致
    pub param_names: Vec<String>,
    /// 自然语言约束，如 "x1 = 0" 或 "x1 > x2"
    pub hypothesis: String,
}

/// 假设检验结果（统一格式，便于前端）
#[derive(Debug, Serialize)]
pub struct HypothesisTestResponse {
    /// 检验类型："t" | "wald"
    pub test_type: String,
    /// 原假设 H0 的线性形式（恒为 Rβ = r）
    pub h0_form: String,
    /// 备择假设 H1 的线性形式（= / ≠ / < / ≤ / > / ≥）
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    /// t 统计量或 F 统计量
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

/// 根据原始约束类型选择 H1 显示符号
fn h1_display_op(h: &HypothesisExpr) -> &'static str {
    match h {
        HypothesisExpr::Eq(_, _) => " ≠ ",
        HypothesisExpr::Lt(_, _) => " < ",
        HypothesisExpr::Le(_, _) => " ≤ ",
        HypothesisExpr::Gt(_, _) => " > ",
        HypothesisExpr::Ge(_, _) => " ≥ ",
    }
}

fn format_linear_forms(
    r: &Array2<f64>,
    r_vec: &Array1<f64>,
    param_names: &[String],
    constraints: &[HypothesisExpr],
) -> (String, String) {
    let mut h0_rows = Vec::new();
    let mut h1_rows = Vec::new();
    for i in 0..r.nrows() {
        let h1_op = constraints.get(i).map(h1_display_op).unwrap_or(" ≠ ");
        let flip = constraints.get(i).map_or(false, |h| {
            matches!(h, HypothesisExpr::Lt(_, _) | HypothesisExpr::Le(_, _))
        });
        let sign = if flip { -1.0 } else { 1.0 };

        let mut terms = Vec::new();
        for j in 0..r.ncols() {
            let c = sign * r[[i, j]];
            if c.abs() < 1e-14 {
                continue;
            }
            let name = param_names.get(j).map(|s| s.as_str()).unwrap_or("?");
            let s = if (c - 1.0).abs() < 1e-10 {
                name.to_string()
            } else if (c + 1.0).abs() < 1e-10 {
                format!("-{}", name)
            } else {
                format!("{:.4}*{}", c, name)
            };
            terms.push((c, s));
        }
        let lhs: String = terms
            .iter()
            .enumerate()
            .map(|(idx, (c, s))| {
                if idx == 0 {
                    if *c < 0.0 {
                        format!("-{}", s.trim_start_matches('-'))
                    } else {
                        s.clone()
                    }
                } else if *c > 0.0 {
                    format!(" + {}", s)
                } else {
                    format!(" - {}", s.trim_start_matches('-'))
                }
            })
            .collect();
        let rhs = sign * r_vec[i];
        let lhs_trim = lhs.trim().to_string();
        h0_rows.push(format!("{} = {:.4}", lhs_trim, rhs));
        h1_rows.push(format!("{}{}{:.4}", lhs_trim, h1_op, rhs));
    }
    (h0_rows.join(" ; "), h1_rows.join(" ; "))
}

fn spec_alternative_to_sci(alt: crate::ast::Alternative) -> SciAlternative {
    match alt {
        crate::ast::Alternative::TwoSided => SciAlternative::TwoSided,
        crate::ast::Alternative::Greater => SciAlternative::Greater,
        crate::ast::Alternative::Less => SciAlternative::Less,
    }
}

fn run_hypothesis_test(req: HypothesisTestRequest) -> Result<HypothesisTestResponse, String> {
    let k = req.betas.len();
    if req.param_names.len() != k {
        return Err(format!(
            "param_names 长度 {} 与 betas 长度 {} 不一致",
            req.param_names.len(),
            k
        ));
    }

    // 1. 解析假设
    let mut param_registry = ParamRegistry::new();
    let constraints = parse_hypothesis_with_registry(&req.hypothesis, &mut param_registry)
        .map_err(|e| format!("解析假设失败: {}", e))?;

    if constraints.is_empty() {
        return Err("至少需要一条约束".to_string());
    }

    // 2. 线性展开
    let test_spec = linear_expand(&constraints).map_err(|e| format!("线性展开失败: {}", e))?;

    let (r, r_vec, alternative, test_method, kind) = match &test_spec.hypothesis {
        HypothesisSpec::Linear { r, r_vec, kind } => (
            r.clone(),
            r_vec.clone(),
            test_spec.alternative,
            test_spec.test_method,
            *kind,
        ),
        HypothesisSpec::Nonlinear { .. } => {
            return Err("非线性约束暂不支持".to_string());
        }
    };

    if test_method == TestMethod::Wald && kind != LinearConstraintKind::Eq {
        return Err("Wald 检验仅支持等式约束 (Rβ = r)，请使用 = 而非 > 或 <".to_string());
    }

    // 3. 参数映射：R 从 ParamId 列序 → OLS 列序
    let param_order = collect_param_order(&constraints);
    let (r_ols, r_vec) =
        reorder_r_to_ols_columns(&r, &r_vec, &param_order, &param_registry, &req.param_names)
            .map_err(|e| format!("参数映射失败: {}", e))?;

    // 4. 构建 ndarray
    let betas = Array1::from_vec(req.betas);
    let cov_beta = Array2::from_shape_vec(
        (k, k),
        req.cov_beta.into_iter().flatten().collect::<Vec<_>>(),
    )
    .map_err(|e| format!("cov_beta 形状错误: {}", e))?;

    // 5. 按检验方法分发
    let (h0_form, h1_form) = format_linear_forms(&r_ols, &r_vec, &req.param_names, &constraints);
    let sci_alt = spec_alternative_to_sci(alternative);

    let response = match test_method {
        TestMethod::TTest => {
            let result = t_test(
                &betas,
                &cov_beta,
                &r_ols,
                &r_vec,
                req.df_residual,
                sci_alt,
                req.hypothesis.clone(),
            )?;
            HypothesisTestResponse {
                test_type: "t".to_string(),
                h0_form: h0_form.clone(),
                h1_form: h1_form.clone(),
                alternative: result.alternative,
                r_beta_minus_r: result.r_beta_minus_r,
                stat: result.stat,
                df1: 1,
                df2: result.df,
                p_value: result.p_value,
            }
        }
        TestMethod::Wald => {
            let result = wald_test(
                &betas,
                &cov_beta,
                &r_ols,
                &r_vec,
                req.df_residual,
                sci_alt,
                req.hypothesis.clone(),
            )?;
            HypothesisTestResponse {
                test_type: "wald".to_string(),
                h0_form,
                h1_form,
                alternative: result.alternative,
                r_beta_minus_r: result.r_beta_minus_r,
                stat: result.stat,
                df1: result.df1,
                df2: result.df2,
                p_value: result.p_value,
            }
        }
    };

    Ok(response)
}

/// Tauri 命令：假设检验
#[tauri::command]
pub fn hypothesis_test(req: HypothesisTestRequest) -> Result<HypothesisTestResponse, String> {
    run_hypothesis_test(req)
}
