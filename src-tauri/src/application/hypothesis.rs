//! Hypothesis testing orchestration: parse → linearize → format → dispatch.

use ndarray::{Array1, Array2};
use std::collections::HashMap;

use crate::math::{
    BinaryOp, ComparisonOp, MathExpr, MathRelation, ParseOptions, UnaryOp, ensure_relation_count,
    parse_relations,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    TwoSided,
    Greater,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMethod {
    TTest,
    Wald,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearConstraintKind {
    Eq,
    Ge,
}

#[derive(Debug)]
pub struct ResolvedLinearHypothesis {
    pub constraints: Vec<MathRelation>,
    pub r_ols: Array2<f64>,
    pub r_vec: Array1<f64>,
    pub alternative: Alternative,
    pub test_method: TestMethod,
    pub kind: LinearConstraintKind,
}

/// Parse and compile constraints directly into OLS `param_names` column order.
pub fn resolve_linear_hypothesis(
    hypothesis: &str,
    param_names: &[String],
) -> Result<ResolvedLinearHypothesis, String> {
    let constraints = parse_relations(hypothesis, ParseOptions::plain(param_names))
        .map_err(|error| format!("解析假设失败: {error}"))?;
    ensure_relation_count(constraints.len()).map_err(|error| format!("解析假设失败: {error}"))?;
    let kind = constraint_kind(&constraints)?;
    if constraints.len() > 1 && kind != LinearConstraintKind::Eq {
        return Err("Wald 检验仅支持等式约束 (Rβ = r)，请使用 = 而非 > 或 <".to_string());
    }

    let mut r_ols = Array2::zeros((constraints.len(), param_names.len()));
    let mut r_vec = Array1::zeros(constraints.len());
    let columns: HashMap<&str, usize> = param_names
        .iter()
        .enumerate()
        .map(|(index, name)| (name.as_str(), index))
        .collect();

    for (row, relation) in constraints.iter().enumerate() {
        let mut form = linearize(&relation.left, &columns)
            .and_then(|left| linearize(&relation.right, &columns).map(|right| left.sub(right)))
            .map_err(|error| format!("线性展开失败: 第 {} 条约束: {error}", row + 1))?;
        let sign = if matches!(relation.op, ComparisonOp::Lt | ComparisonOp::Le) {
            -1.0
        } else {
            1.0
        };
        form.scale(sign);
        for (column, coefficient) in form.coefficients {
            r_ols[[row, column]] = coefficient;
        }
        r_vec[row] = -form.constant;
    }

    let test_method = if constraints.len() == 1 {
        TestMethod::TTest
    } else {
        TestMethod::Wald
    };
    let alternative = if kind == LinearConstraintKind::Eq {
        Alternative::TwoSided
    } else {
        Alternative::Greater
    };
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
    let sci_alt = match resolved.alternative {
        Alternative::TwoSided => SciAlternative::TwoSided,
        Alternative::Greater => SciAlternative::Greater,
    };
    let betas = Array1::from_vec(input.betas);
    let cov_beta = Array2::from_shape_vec(
        (k, k),
        input.cov_beta.into_iter().flatten().collect::<Vec<_>>(),
    )
    .map_err(|error| format!("cov_beta 形状错误: {error}"))?;

    let test_input = LinearHypothesisTestInput {
        betas: &betas,
        cov_beta: &cov_beta,
        r: &resolved.r_ols,
        r_vec: &resolved.r_vec,
        df_residual: input.df_residual,
        alternative: sci_alt,
        constraint_desc: input.hypothesis,
    };
    match resolved.test_method {
        TestMethod::TTest => {
            let result =
                t_test(&SciContext::rust(), test_input).map_err(|error| error.to_string())?;
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
            let result =
                wald_test(&SciContext::rust(), test_input).map_err(|error| error.to_string())?;
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
    if at_spec.trim().is_empty() {
        return Ok(HashMap::new());
    }
    let resolved = resolve_linear_hypothesis(at_spec, param_names)?;
    if resolved.kind != LinearConstraintKind::Eq {
        return Err("at() 仅支持线性等式约束 (param = value)".to_string());
    }

    let mut values = HashMap::new();
    for row_index in 0..resolved.r_ols.nrows() {
        let row = resolved.r_ols.row(row_index);
        let columns = row
            .iter()
            .enumerate()
            .filter(|(_, coefficient)| coefficient.abs() > 1e-14)
            .collect::<Vec<_>>();
        if columns.len() != 1 {
            return Err(format!(
                "at() 约束 {} 必须是简单形式 param = value",
                row_index + 1
            ));
        }
        let (column, coefficient) = columns[0];
        if coefficient.abs() <= f64::EPSILON {
            return Err(format!("at() 约束 {} 的参数系数不能为零", row_index + 1));
        }
        values.insert(
            param_names[column].clone(),
            resolved.r_vec[row_index] / coefficient,
        );
    }
    Ok(values)
}

#[derive(Default)]
struct LinearForm {
    coefficients: HashMap<usize, f64>,
    constant: f64,
}

impl LinearForm {
    fn number(value: f64) -> Self {
        Self {
            coefficients: HashMap::new(),
            constant: value,
        }
    }

    fn add(mut self, other: Self) -> Self {
        for (column, coefficient) in other.coefficients {
            *self.coefficients.entry(column).or_default() += coefficient;
        }
        self.constant += other.constant;
        self
    }

    fn sub(self, mut other: Self) -> Self {
        other.scale(-1.0);
        self.add(other)
    }

    fn scale(&mut self, factor: f64) {
        for coefficient in self.coefficients.values_mut() {
            *coefficient *= factor;
        }
        self.constant *= factor;
    }

    fn is_constant(&self) -> bool {
        self.coefficients.values().all(|value| value.abs() <= 1e-14)
    }
}

fn linearize(expr: &MathExpr, columns: &HashMap<&str, usize>) -> Result<LinearForm, String> {
    match expr {
        MathExpr::Number(value) => Ok(LinearForm::number(*value)),
        MathExpr::Symbol(name) => {
            let column = columns
                .get(name.as_str())
                .copied()
                .ok_or_else(|| format!("假设中的参数 '{name}' 不在 param_names 中"))?;
            Ok(LinearForm {
                coefficients: HashMap::from([(column, 1.0)]),
                constant: 0.0,
            })
        }
        MathExpr::Unary {
            op: UnaryOp::Neg,
            operand,
        } => {
            let mut form = linearize(operand, columns)?;
            form.scale(-1.0);
            Ok(form)
        }
        MathExpr::Binary {
            op: BinaryOp::Add,
            left,
            right,
        } => Ok(linearize(left, columns)?.add(linearize(right, columns)?)),
        MathExpr::Binary {
            op: BinaryOp::Sub,
            left,
            right,
        } => Ok(linearize(left, columns)?.sub(linearize(right, columns)?)),
        MathExpr::Binary {
            op: BinaryOp::Mul,
            left,
            right,
        } => {
            let left = linearize(left, columns)?;
            let right = linearize(right, columns)?;
            match (left.is_constant(), right.is_constant()) {
                (true, _) => {
                    let mut right = right;
                    right.scale(left.constant);
                    Ok(right)
                }
                (_, true) => {
                    let mut left = left;
                    left.scale(right.constant);
                    Ok(left)
                }
                _ => Err("乘法至少一侧必须是常数".to_string()),
            }
        }
        MathExpr::Binary {
            op: BinaryOp::Div,
            left,
            right,
        } => {
            let mut left = linearize(left, columns)?;
            let right = linearize(right, columns)?;
            if !right.is_constant() {
                return Err("除法仅支持除以常数".to_string());
            }
            if right.constant.abs() <= f64::EPSILON {
                return Err("除数不能为零".to_string());
            }
            left.scale(1.0 / right.constant);
            Ok(left)
        }
        MathExpr::Binary {
            op: BinaryOp::Pow, ..
        } => Err("幂运算不是线性表达式".to_string()),
        MathExpr::Call { name, .. } => Err(format!("函数 {name}() 不是线性表达式")),
    }
}

fn constraint_kind(relations: &[MathRelation]) -> Result<LinearConstraintKind, String> {
    if relations
        .iter()
        .any(|relation| relation.op == ComparisonOp::DistributedAs)
    {
        return Err("假设约束不支持分布关系 ~".to_string());
    }
    let first = relations
        .first()
        .ok_or_else(|| "至少需要一条约束".to_string())?;
    let direction = relation_direction(first.op);
    for (index, relation) in relations.iter().enumerate().skip(1) {
        if relation_direction(relation.op) != direction {
            return Err(format!(
                "不等式方向必须一致，第 {} 条约束方向不一致",
                index + 1
            ));
        }
    }
    Ok(if direction == 0 {
        LinearConstraintKind::Eq
    } else {
        LinearConstraintKind::Ge
    })
}

fn relation_direction(op: ComparisonOp) -> i8 {
    match op {
        ComparisonOp::Eq => 0,
        ComparisonOp::Lt | ComparisonOp::Le => -1,
        ComparisonOp::Gt | ComparisonOp::Ge => 1,
        ComparisonOp::DistributedAs => 2,
    }
}

fn h1_display_op(relation: &MathRelation) -> &'static str {
    match relation.op {
        ComparisonOp::Eq => " ≠ ",
        ComparisonOp::Lt => " < ",
        ComparisonOp::Le => " ≤ ",
        ComparisonOp::Gt => " > ",
        ComparisonOp::Ge => " ≥ ",
        ComparisonOp::DistributedAs => " ∼ ",
    }
}

fn format_linear_forms(
    r: &Array2<f64>,
    r_vec: &Array1<f64>,
    param_names: &[String],
    constraints: &[MathRelation],
) -> (String, String) {
    let mut h0_rows = Vec::new();
    let mut h1_rows = Vec::new();
    for row in 0..r.nrows() {
        let relation = &constraints[row];
        let sign = if matches!(relation.op, ComparisonOp::Lt | ComparisonOp::Le) {
            -1.0
        } else {
            1.0
        };
        let mut terms = Vec::new();
        for column in 0..r.ncols() {
            let coefficient = sign * r[[row, column]];
            if coefficient.abs() < 1e-14 {
                continue;
            }
            let name = &param_names[column];
            let text = if (coefficient - 1.0).abs() < 1e-10 {
                name.clone()
            } else if (coefficient + 1.0).abs() < 1e-10 {
                format!("-{name}")
            } else {
                format!("{coefficient:.4}*{name}")
            };
            terms.push((coefficient, text));
        }
        let lhs = terms
            .iter()
            .enumerate()
            .map(|(index, (coefficient, text))| {
                if index == 0 {
                    text.clone()
                } else if *coefficient > 0.0 {
                    format!(" + {text}")
                } else {
                    format!(" - {}", text.trim_start_matches('-'))
                }
            })
            .collect::<String>();
        let rhs = sign * r_vec[row];
        h0_rows.push(format!("{} = {:.4}", lhs.trim(), rhs));
        h1_rows.push(format!(
            "{}{}{:.4}",
            lhs.trim(),
            h1_display_op(relation),
            rhs
        ));
    }
    (h0_rows.join(" ; "), h1_rows.join(" ; "))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names() -> Vec<String> {
        vec!["x1".into(), "x2".into()]
    }

    #[test]
    fn resolves_single_equality_directly_in_param_order() {
        let resolved = resolve_linear_hypothesis("x2 - 2*x1 = 3", &names()).unwrap();
        assert_eq!(resolved.test_method, TestMethod::TTest);
        assert_eq!(resolved.r_ols.row(0).to_vec(), vec![-2.0, 1.0]);
        assert_eq!(resolved.r_vec[0], 3.0);
    }

    #[test]
    fn supports_equality_alias_chains_and_wald() {
        let resolved = resolve_linear_hypothesis("x1 == 0, x2 = 1", &names()).unwrap();
        assert_eq!(resolved.test_method, TestMethod::Wald);
        assert_eq!(resolved.r_ols, Array2::<f64>::eye(2));
    }

    #[test]
    fn flips_less_direction_and_preserves_display() {
        let resolved = resolve_linear_hypothesis("x1 < 2", &names()).unwrap();
        assert_eq!(resolved.alternative, Alternative::Greater);
        assert_eq!(resolved.r_ols[[0, 0]], -1.0);
        assert_eq!(resolved.r_vec[0], -2.0);
        let (_, h1) = format_linear_forms(
            &resolved.r_ols,
            &resolved.r_vec,
            &names(),
            &resolved.constraints,
        );
        assert_eq!(h1, "x1 < 2.0000");
    }

    #[test]
    fn parses_chain_before_rejecting_multi_inequality_wald() {
        let error = resolve_linear_hypothesis("0 < x1 < 2", &names()).unwrap_err();
        assert!(error.contains("Wald 检验仅支持等式约束"));
    }

    #[test]
    fn rejects_excessive_constraints_before_matrix_allocation() {
        let input = std::iter::repeat_n("x1 = 0", crate::math::MAX_RELATIONS + 1)
            .collect::<Vec<_>>()
            .join(", ");
        let error = resolve_linear_hypothesis(&input, &names()).unwrap_err();
        assert!(error.contains("关系数量不能超过"));
    }

    #[test]
    fn rejects_zero_divisor_and_nonlinear_calls() {
        assert!(
            resolve_linear_hypothesis("x1 / 0 = 1", &names())
                .unwrap_err()
                .contains("除数不能为零")
        );
        assert!(
            resolve_linear_hypothesis("exp(x1) = 2", &names())
                .unwrap_err()
                .contains("不是线性表达式")
        );
        assert!(
            resolve_linear_hypothesis("ln(x1) = 2", &names())
                .unwrap_err()
                .contains("不是线性表达式")
        );
    }

    #[test]
    fn parses_at_values_from_equalities() {
        let values = parse_at_values("x1 = 0, x2 = 1.5", &names()).unwrap();
        assert_eq!(values["x1"], 0.0);
        assert_eq!(values["x2"], 1.5);
    }

    #[test]
    fn rejects_non_simple_at_constraint() {
        assert!(
            parse_at_values("x1 + x2 = 1", &names())
                .unwrap_err()
                .contains("简单形式")
        );
    }
}
