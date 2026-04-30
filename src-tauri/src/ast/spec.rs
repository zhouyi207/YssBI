//! 假设检验规格
//!
//! 将验证后的 HypothesisExpr 转化为可执行的 TestSpec（R·β = r 形式）。

use std::collections::HashMap;

use ndarray::{Array1, Array2};

use crate::ast::parser::ParamRegistry;
use crate::ast::types::{Expr, HypothesisExpr, ParamId};
use crate::ast::validator::ConstraintDirection;
use crate::ast::ValidationError;

/// 备择假设类型
///
/// - TwoSided: H1: g(β) ≠ 0
/// - Greater:  H1: g(β) > 0
/// - Less:     H1: g(β) < 0
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    /// H1: g(β) ≠ 0
    TwoSided,
    /// H1: g(β) > 0
    Greater,
    /// H1: g(β) < 0
    Less,
}

/// 线性约束类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinearConstraintKind {
    /// R·β = r
    Eq,
    /// R·β ≥ r（已统一，Lt/Le 会翻转为该形式）
    Ge,
}

/// 假设规格
#[derive(Debug, Clone)]
pub enum HypothesisSpec {
    /// 线性约束
    /// R: 约束矩阵 (n_constraints × n_params)
    /// r: 约束向量 (n_constraints)
    Linear {
        /// 约束矩阵
        r: Array2<f64>,
        /// 约束向量（右端）
        r_vec: Array1<f64>,
        /// Eq: R·β=r, Ge: R·β≥r
        kind: LinearConstraintKind,
    },

    /// 非线性约束: g(β) = 0
    /// 预留扩展，暂不实现
    #[allow(dead_code)]
    Nonlinear {
        // g: Fn(β) -> Vector, J: Fn(β) -> Matrix
        // 在 Rust 中可用 Box<dyn Fn(&Array1<f64>) -> Array1<f64>> 等
        _placeholder: (),
    },
}

/// 检验方法
///
/// - TTest: 单约束 (q=1)，t 检验，支持单侧
/// - Wald: 多约束 (q>1)，Wald F 检验
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestMethod {
    TTest,
    Wald,
}

/// 假设检验规格
///
/// 组合假设形式、备择假设与检验方法。
#[derive(Debug, Clone)]
pub struct TestSpec {
    pub hypothesis: HypothesisSpec,
    pub alternative: Alternative,
    /// 根据约束个数自动选择：q=1 → TTest，q>1 → Wald
    pub test_method: TestMethod,
}

/// 线性展开错误
#[derive(Debug, Clone, PartialEq)]
pub enum LinearExpandError {
    /// 约束为空
    EmptyConstraints,
    /// 验证失败
    Validation(ValidationError),
}

impl std::fmt::Display for LinearExpandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinearExpandError::EmptyConstraints => write!(f, "至少需要一条约束"),
            LinearExpandError::Validation(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for LinearExpandError {}

impl From<ValidationError> for LinearExpandError {
    fn from(e: ValidationError) -> Self {
        LinearExpandError::Validation(e)
    }
}

/// 将 Expr 展开为线性形式: sum_i c_i·β_i + constant
fn expr_to_linear_form(expr: &Expr) -> (HashMap<ParamId, f64>, f64) {
    match expr {
        Expr::Const(c) => (HashMap::new(), *c),
        Expr::Param(id) => {
            let mut m = HashMap::new();
            m.insert(*id, 1.0);
            (m, 0.0)
        }
        Expr::Add(l, r) => {
            let (mut l_c, l_k) = expr_to_linear_form(l);
            let (r_c, r_k) = expr_to_linear_form(r);
            for (id, v) in r_c {
                *l_c.entry(id).or_insert(0.0) += v;
            }
            (l_c, l_k + r_k)
        }
        Expr::Sub(l, r) => {
            let (mut l_c, l_k) = expr_to_linear_form(l);
            let (r_c, r_k) = expr_to_linear_form(r);
            for (id, v) in r_c {
                *l_c.entry(id).or_insert(0.0) -= v;
            }
            (l_c, l_k - r_k)
        }
        Expr::Mul(k, e) => {
            let (c, konst) = expr_to_linear_form(e);
            let coeffs = c.into_iter().map(|(id, v)| (id, k * v)).collect();
            (coeffs, k * konst)
        }
        Expr::Div(l, r) => {
            let (c, konst) = expr_to_linear_form(l);
            let (_, r_konst) = expr_to_linear_form(r);
            let scale = 1.0 / r_konst;
            let coeffs = c.into_iter().map(|(id, v)| (id, scale * v)).collect();
            (coeffs, scale * konst)
        }
        Expr::Exp(_) | Expr::Log(_) => unreachable!("linear expr only"),
    }
}

/// 收集约束中出现的所有 ParamId，按 ParamId.0 排序
pub fn collect_param_order(constraints: &[HypothesisExpr]) -> Vec<ParamId> {
    let mut ids: Vec<ParamId> = constraints
        .iter()
        .flat_map(|h| {
            let expr = match h {
                HypothesisExpr::Eq(e, _)
                | HypothesisExpr::Lt(e, _)
                | HypothesisExpr::Le(e, _)
                | HypothesisExpr::Gt(e, _)
                | HypothesisExpr::Ge(e, _) => e,
            };
            collect_params_from_expr(expr)
        })
        .collect();
    ids.sort_by_key(|id| id.0);
    ids.dedup();
    ids
}

fn collect_params_from_expr(expr: &Expr) -> Vec<ParamId> {
    match expr {
        Expr::Param(id) => vec![*id],
        Expr::Add(l, r) | Expr::Sub(l, r) | Expr::Div(l, r) => {
            let mut v = collect_params_from_expr(l);
            v.extend(collect_params_from_expr(r));
            v
        }
        Expr::Mul(_, e) => collect_params_from_expr(e),
        Expr::Const(_) | Expr::Exp(_) | Expr::Log(_) => vec![],
    }
}

/// 将验证后的 HypothesisExpr 线性展开为 HypothesisSpec::Linear
///
/// 要求：Expr 线性，且所有约束方向一致。
/// Lt/Le 会翻转为 R·β ≥ r 形式（-R·β ≥ -r）。
/// Alternative：Eq→TwoSided；Ge→Greater；Le→Less。
/// 参数顺序按 ParamId 升序排列。
pub fn linear_expand(constraints: &[HypothesisExpr]) -> Result<TestSpec, LinearExpandError> {
    crate::ast::validate_hypotheses(constraints)?;

    if constraints.is_empty() {
        return Err(LinearExpandError::EmptyConstraints);
    }

    let dir = constraints[0].direction();
    let alternative = match dir {
        ConstraintDirection::Eq => Alternative::TwoSided,
        // Ge: expr ≥ k，检验 H1: expr > k，即 contrast = Rβ - r > 0
        ConstraintDirection::Ge => Alternative::Greater,
        // Le/Lt: 翻转后 Rβ = -expr, r = -k，用户 "expr < k" 即 H1: expr < k ↔ -expr > -k ↔ Rβ > r
        ConstraintDirection::Le => Alternative::Greater,
    };

    let param_order = collect_param_order(constraints);
    let n_params = param_order.len();
    let param_to_col: HashMap<ParamId, usize> = param_order
        .iter()
        .enumerate()
        .map(|(i, &id)| (id, i))
        .collect();

    let n_constraints = constraints.len();
    let mut r_mat = Array2::zeros((n_constraints, n_params));
    let mut r_vec = Array1::zeros(n_constraints);

    for (i, h) in constraints.iter().enumerate() {
        let (expr, k) = match h {
            HypothesisExpr::Eq(e, rhs) => (e, *rhs),
            HypothesisExpr::Lt(e, rhs)
            | HypothesisExpr::Le(e, rhs)
            | HypothesisExpr::Gt(e, rhs)
            | HypothesisExpr::Ge(e, rhs) => (e, *rhs),
        };

        let (coeffs, constant) = expr_to_linear_form(expr);
        // expr op k  =>  sum_j c_j·β_j + constant op k  =>  sum_j c_j·β_j op k - constant
        let rhs = k - constant;

        let flip = matches!(h, HypothesisExpr::Lt(_, _) | HypothesisExpr::Le(_, _));
        let sign = if flip { -1.0 } else { 1.0 };

        for (id, c) in coeffs {
            if let Some(&col) = param_to_col.get(&id) {
                r_mat[[i, col]] = sign * c;
            }
        }
        r_vec[i] = sign * rhs;
    }

    let kind = match dir {
        ConstraintDirection::Eq => LinearConstraintKind::Eq,
        ConstraintDirection::Ge | ConstraintDirection::Le => LinearConstraintKind::Ge,
    };

    let test_method = if n_constraints == 1 {
        TestMethod::TTest
    } else {
        TestMethod::Wald
    };

    Ok(TestSpec {
        hypothesis: HypothesisSpec::Linear {
            r: r_mat,
            r_vec,
            kind,
        },
        alternative,
        test_method,
    })
}

/// 将 R 从 ParamId 列序重排为 OLS exog 列序
///
/// - param_order: R 的列 j 对应 param_order[j]
/// - param_names: OLS 的列 i 对应 param_names[i]
/// - 返回 (R_ols, r_vec)，r_vec 不变
pub fn reorder_r_to_ols_columns(
    r: &Array2<f64>,
    r_vec: &Array1<f64>,
    param_order: &[ParamId],
    param_registry: &ParamRegistry,
    param_names: &[impl AsRef<str>],
) -> Result<(Array2<f64>, Array1<f64>), String> {
    let q = r.nrows();
    if param_order.len() != r.ncols() {
        return Err(format!(
            "param_order 长度 {} 与 R 列数 {} 不一致",
            param_order.len(),
            r.ncols()
        ));
    }

    let k_ols = param_names.len();
    let mut r_ols = Array2::zeros((q, k_ols));

    for j in 0..param_order.len() {
        let name = param_registry
            .get_name(param_order[j])
            .ok_or_else(|| format!("参数 ID {:?} 在 registry 中无名称", param_order[j]))?;
        let ols_col = param_names
            .iter()
            .position(|n| n.as_ref() == name)
            .ok_or_else(|| format!("假设中的参数 '{}' 不在 param_names 中", name))?;
        for i in 0..q {
            r_ols[[i, ols_col]] = r[[i, j]];
        }
    }

    Ok((r_ols, r_vec.clone()))
}
