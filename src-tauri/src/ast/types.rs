//! 假设表达式 IR 类型定义
//!
//! 将数学约束（如 s = 0.1, s > 0, exp(s) = 2）表示为可计算的 AST。

use serde::{Deserialize, Serialize};

/// 参数标识符
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParamId(pub u32);

/// 假设约束表达式
///
/// 左端为 Expr，右端为常数。支持严格/非严格不等号。
#[derive(Debug, Clone, PartialEq)]
pub enum HypothesisExpr {
    /// 相等: expr = k
    Eq(Expr, f64),
    /// 小于: expr < k（严格）
    Lt(Expr, f64),
    /// 小于等于: expr <= k
    Le(Expr, f64),
    /// 大于: expr > k（严格）
    Gt(Expr, f64),
    /// 大于等于: expr >= k
    Ge(Expr, f64),
}

/// 算术表达式
///
/// Mul 当前仅支持 k * expr，预留 Expr * Expr 扩展。
#[derive(Debug, Clone, PartialEq)]
pub enum Expr {
    /// 常数
    Const(f64),
    /// 参数
    Param(ParamId),
    /// 加法
    Add(Box<Expr>, Box<Expr>),
    /// 减法
    Sub(Box<Expr>, Box<Expr>),
    /// 标量乘法: k * expr（当前仅支持此形式）
    /// 预留: 可扩展为 Mul(Box<Expr>, Box<Expr>)
    Mul(f64, Box<Expr>),
    /// 除法: expr1 / expr2
    Div(Box<Expr>, Box<Expr>),
    /// 指数: exp(expr)
    Exp(Box<Expr>),
    /// 对数: log(expr)
    Log(Box<Expr>),
}

impl Expr {
    /// 构造常数表达式
    pub fn const_(v: f64) -> Self {
        Expr::Const(v)
    }

    /// 构造参数表达式
    pub fn param(id: ParamId) -> Self {
        Expr::Param(id)
    }

    /// 构造加法
    pub fn add(l: Expr, r: Expr) -> Self {
        Expr::Add(Box::new(l), Box::new(r))
    }

    /// 构造减法
    pub fn sub(l: Expr, r: Expr) -> Self {
        Expr::Sub(Box::new(l), Box::new(r))
    }

    /// 构造标量乘法 k * expr
    pub fn mul_scalar(k: f64, e: Expr) -> Self {
        Expr::Mul(k, Box::new(e))
    }

    /// 构造除法
    pub fn div(l: Expr, r: Expr) -> Self {
        Expr::Div(Box::new(l), Box::new(r))
    }

    /// 构造指数
    pub fn exp(e: Expr) -> Self {
        Expr::Exp(Box::new(e))
    }

    /// 构造对数
    pub fn log(e: Expr) -> Self {
        Expr::Log(Box::new(e))
    }
}
