//! 假设表达式验证
//!
//! 支持等式及线性不等式，要求所有不等式方向一致（不能混用 s>0 与 t<0）。

use crate::ast::types::{Expr, HypothesisExpr};

/// 约束方向（用于检查一致性）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConstraintDirection {
    Eq,
    Ge, // > 或 >=
    Le, // < 或 <=
}

impl HypothesisExpr {
    /// 获取约束方向
    pub fn direction(&self) -> ConstraintDirection {
        match self {
            HypothesisExpr::Eq(_, _) => ConstraintDirection::Eq,
            HypothesisExpr::Gt(_, _) | HypothesisExpr::Ge(_, _) => ConstraintDirection::Ge,
            HypothesisExpr::Lt(_, _) | HypothesisExpr::Le(_, _) => ConstraintDirection::Le,
        }
    }
}

/// 验证错误
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationError {
    /// 不等式方向不一致（如同时出现 s>0 与 t<0）
    MixedDirection(String),
    /// 不支持非线性表达式（如 exp、log）
    NonLinearExprNotSupported(String),
    /// 除法仅支持除以常数
    DivByNonConstNotSupported(String),
    /// 第 N 条约束验证失败
    ConstraintFailed { index: usize, message: String },
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValidationError::MixedDirection(msg) => {
                write!(f, "不等式方向必须一致，不能混用 > 与 <: {}", msg)
            }
            ValidationError::NonLinearExprNotSupported(msg) => {
                write!(f, "不支持非线性表达式（如 exp、log）: {}", msg)
            }
            ValidationError::DivByNonConstNotSupported(msg) => {
                write!(f, "除法仅支持除以常数: {}", msg)
            }
            ValidationError::ConstraintFailed { index, message } => {
                write!(f, "第 {} 条约束: {}", index, message)
            }
        }
    }
}

impl std::error::Error for ValidationError {}

/// 验证单个假设约束的 Expr 为线性
pub fn validate_hypothesis(h: &HypothesisExpr) -> Result<(), ValidationError> {
    let expr = match h {
        HypothesisExpr::Eq(e, _)
        | HypothesisExpr::Lt(e, _)
        | HypothesisExpr::Le(e, _)
        | HypothesisExpr::Gt(e, _)
        | HypothesisExpr::Ge(e, _) => e,
    };
    validate_expr_linear(expr)
}

/// 验证一组约束：Expr 线性 + 方向一致
pub fn validate_hypotheses(constraints: &[HypothesisExpr]) -> Result<(), ValidationError> {
    if constraints.is_empty() {
        return Ok(());
    }

    let first_dir = constraints[0].direction();
    for (i, h) in constraints.iter().enumerate() {
        validate_hypothesis(h).map_err(|e| ValidationError::ConstraintFailed {
            index: i + 1,
            message: e.to_string(),
        })?;

        let dir = h.direction();
        if dir != first_dir {
            return Err(ValidationError::MixedDirection(format!(
                "第 {} 条约束方向 ({}) 与第 1 条 ({}) 不一致",
                i + 1,
                direction_desc(dir),
                direction_desc(first_dir)
            )));
        }
    }
    Ok(())
}

fn direction_desc(d: ConstraintDirection) -> &'static str {
    match d {
        ConstraintDirection::Eq => "=",
        ConstraintDirection::Ge => "> 或 >=",
        ConstraintDirection::Le => "< 或 <=",
    }
}

/// 验证 Expr 为线性
///
/// 线性：Const、Param、Add、Sub、Mul(k, expr)、Div(expr, Const)
/// 非线性：Exp、Log、Div(expr, 非Const)
fn validate_expr_linear(expr: &Expr) -> Result<(), ValidationError> {
    match expr {
        Expr::Const(_) | Expr::Param(_) => Ok(()),
        Expr::Add(l, r) | Expr::Sub(l, r) => {
            validate_expr_linear(l)?;
            validate_expr_linear(r)
        }
        Expr::Mul(_, e) => validate_expr_linear(e),
        Expr::Div(l, r) => {
            validate_expr_linear(l)?;
            match r.as_ref() {
                Expr::Const(_) => Ok(()),
                _ => Err(ValidationError::DivByNonConstNotSupported(
                    "除数必须为常数".to_string(),
                )),
            }
        }
        Expr::Exp(_) => Err(ValidationError::NonLinearExprNotSupported(
            "exp() 暂不支持".to_string(),
        )),
        Expr::Log(_) => Err(ValidationError::NonLinearExprNotSupported(
            "log() 暂不支持".to_string(),
        )),
    }
}
