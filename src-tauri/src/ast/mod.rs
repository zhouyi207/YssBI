//! 假设表达式 AST 与解析
//!
//! 将自由文本形式的数学约束（如 `s = 0.1`, `s > 0`, `exp(s) = 2`）
//! 解析为可计算的 IR（HypothesisExpr / Expr）。

pub mod lexer;
pub mod parser;
pub mod spec;
pub mod types;
pub mod validator;

#[cfg(test)]
mod test;

pub use lexer::{LexError, Lexer, Token};
pub use parser::{ParamRegistry, ParseError, parse_hypothesis, parse_hypothesis_with_registry};
pub use spec::{
    Alternative, HypothesisSpec, LinearConstraintKind, LinearExpandError, TestMethod, TestSpec,
    collect_param_order, linear_expand, reorder_r_to_ols_columns,
};
pub use types::{Expr, HypothesisExpr, ParamId};
pub use validator::{
    ConstraintDirection, ValidationError, validate_hypotheses, validate_hypothesis,
};
