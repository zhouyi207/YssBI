//! Hypothesis testing orchestration: parse → linearize → map columns → format → dispatch.

use ndarray::{Array1, Array2};
use std::collections::HashMap;

use crate::ast::{
    Alternative, HypothesisExpr, HypothesisSpec, LinearConstraintKind, ParamRegistry, TestMethod,
    collect_param_order, linear_expand, parse_hypothesis_with_registry, reorder_r_to_ols_columns,
};
use crate::sci::api::stats::hypothesis::{
    Alternative as SciAlternative, LinearHypothesisTestInput, t_test, wald_test,
};
use crate::sci::engine::SciContext;

pub struct HypothesisTestInput {
    pub betas: Vec<f64>,
    pub cov_beta: Vec<Vec<f64>>,
    pub df_residual: usize,
    pub param_names: Vec<String>,
    pub hypothesis: String,
}

pub struct HypothesisTestOutput {
    pub test_type: String,
    pub h0_form: String,
    pub h1_form: String,
    pub alternative: String,
    pub r_beta_minus_r: f64,
    pub stat: f64,
    pub df1: usize,
    pub df2: usize,
    pub p_value: f64,
}

pub struct ResolvedLinearHypothesis {
    pub constraints: Vec<HypothesisExpr>,
    pub r_ols: Array2<f64>,
    pub r_vec: Array1<f64>,
    pub alternative: Alternative,
    pub test_method: TestMethod,
    pub kind: LinearConstraintKind,
}

/// Parse a natural-language hypothesis, linearize it, and map R/r into OLS column order.
pub fn resolve_linear_hypothesis(
    hypothesis: &str,
    param_names: &[String],
) -> Result<ResolvedLinearHypothesis, String> {
    let mut param_registry = ParamRegistry::new();
    let constraints = parse_hypothesis_with_registry(hypothesis, &mut param_registry)
        .map_err(|e| format!("解析假设失败: {}", e))?;

    if constraints.is_empty() {
        return Err("至少需要一条约束".to_string());
    }

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

    let param_order = collect_param_order(&constraints);
    let (r_ols, r_vec) =
        reorder_r_to_ols_columns(&r, &r_vec, &param_order, &param_registry, param_names)
            .map_err(|e| format!("参数映射失败: {}", e))?;

    Ok(ResolvedLinearHypothesis {
        constraints,
        r_ols,
        r_vec,
        alternative,
        test_method,
        kind,
    })
}

pub fn run_hypothesis_test(input: HypothesisTestInput) -> Result<HypothesisTestOutput, String> {
    let k = input.betas.len();
    if input.param_names.len() != k {
        return Err(format!(
            "param_names 长度 {} 与 betas 长度 {} 不一致",
            input.param_names.len(),
            k
        ));
    }

    let resolved = resolve_linear_hypothesis(&input.hypothesis, &input.param_names)?;
    let (h0_form, h1_form) = format_linear_forms(
        &resolved.r_ols,
        &resolved.r_vec,
        &input.param_names,
        &resolved.constraints,
    );
    let sci_alt = spec_alternative_to_sci(resolved.alternative);

    let betas = Array1::from_vec(input.betas);
    let cov_beta = Array2::from_shape_vec(
        (k, k),
        input.cov_beta.into_iter().flatten().collect::<Vec<_>>(),
    )
    .map_err(|e| format!("cov_beta 形状错误: {}", e))?;

    match resolved.test_method {
        TestMethod::TTest => {
            let result = t_test(
                &SciContext::rust(),
                LinearHypothesisTestInput {
                    betas: &betas,
                    cov_beta: &cov_beta,
                    r: &resolved.r_ols,
                    r_vec: &resolved.r_vec,
                    df_residual: input.df_residual,
                    alternative: sci_alt,
                    constraint_desc: input.hypothesis,
                },
            )
            .map_err(|error| error.to_string())?;
            Ok(HypothesisTestOutput {
                test_type: "t".to_string(),
                h0_form,
                h1_form,
                alternative: result.alternative,
                r_beta_minus_r: result.r_beta_minus_r,
                stat: result.stat,
                df1: 1,
                df2: result.df,
                p_value: result.p_value,
            })
        }
        TestMethod::Wald => {
            let result = wald_test(
                &SciContext::rust(),
                LinearHypothesisTestInput {
                    betas: &betas,
                    cov_beta: &cov_beta,
                    r: &resolved.r_ols,
                    r_vec: &resolved.r_vec,
                    df_residual: input.df_residual,
                    alternative: sci_alt,
                    constraint_desc: input.hypothesis,
                },
            )
            .map_err(|error| error.to_string())?;
            Ok(HypothesisTestOutput {
                test_type: "wald".to_string(),
                h0_form,
                h1_form,
                alternative: result.alternative,
                r_beta_minus_r: result.r_beta_minus_r,
                stat: result.stat,
                df1: result.df1,
                df2: result.df2,
                p_value: result.p_value,
            })
        }
    }
}

/// Parse margins `at()` specs such as `x1 = 0, x2 = 1.5` into param -> value.
pub fn parse_at_values(
    at_spec: &str,
    param_names: &[String],
) -> Result<HashMap<String, f64>, String> {
    let trimmed = at_spec.trim();
    if trimmed.is_empty() {
        return Ok(HashMap::new());
    }

    let resolved = resolve_linear_hypothesis(trimmed, param_names)?;
    if resolved.kind != LinearConstraintKind::Eq {
        return Err("at() 仅支持线性等式约束 (param = value)".to_string());
    }

    let mut values = HashMap::new();
    for i in 0..resolved.r_ols.nrows() {
        let row = resolved.r_ols.row(i);
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
            let param_name = param_names[j].clone();
            let coeff = resolved.r_ols[[i, j]];
            let value = resolved.r_vec[i] / coeff;
            values.insert(param_name, value);
        }
    }

    Ok(values)
}

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
        let flip = constraints
            .get(i)
            .is_some_and(|h| matches!(h, HypothesisExpr::Lt(_, _) | HypothesisExpr::Le(_, _)));
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

fn spec_alternative_to_sci(alt: Alternative) -> SciAlternative {
    match alt {
        Alternative::TwoSided => SciAlternative::TwoSided,
        Alternative::Greater => SciAlternative::Greater,
        Alternative::Less => SciAlternative::Less,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_single_equality_constraint() {
        let resolved = resolve_linear_hypothesis("x1 = 0", &["x1".into(), "x2".into()])
            .expect("single equality should resolve");
        assert_eq!(resolved.test_method, TestMethod::TTest);
        assert_eq!(resolved.r_ols.nrows(), 1);
    }

    #[test]
    fn parse_at_values_from_equalities() {
        let values = parse_at_values("x1 = 0, x2 = 1.5", &["x1".into(), "x2".into()])
            .expect("at values should parse");
        assert!((values["x1"] - 0.0).abs() < 1e-10);
        assert!((values["x2"] - 1.5).abs() < 1e-10);
    }
}
