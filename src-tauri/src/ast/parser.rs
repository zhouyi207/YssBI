//! 语法分析器：将 Token 流解析为 HypothesisExpr
//!
//! 支持：
//! - 自由文本输入
//! - expr1 op expr2
//! - 链式比较 expr1 op expr2 op ... op exprN
//! - 严格/非严格不等号区分

use std::collections::HashMap;

use crate::ast::lexer::{LexError, Lexer, Token};
use crate::ast::types::{Expr, HypothesisExpr, ParamId};

/// 参数名到 ID 的注册表
#[derive(Debug, Default, Clone)]
pub struct ParamRegistry {
    name_to_id: HashMap<String, ParamId>,
    next_id: u32,
}

impl ParamRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// 获取或创建参数 ID
    pub fn get_or_insert(&mut self, name: &str) -> ParamId {
        if let Some(&id) = self.name_to_id.get(name) {
            id
        } else {
            let id = ParamId(self.next_id);
            self.next_id += 1;
            self.name_to_id.insert(name.to_string(), id);
            id
        }
    }

    /// 获取参数名（用于调试）
    pub fn get_name(&self, id: ParamId) -> Option<&str> {
        self.name_to_id
            .iter()
            .find(|&(_, &v)| v == id)
            .map(|(k, _)| k.as_str())
    }
}

/// 语法分析器
pub struct Parser<'a> {
    lexer: Lexer<'a>,
    current: Option<Token>,
    param_registry: &'a mut ParamRegistry,
}

/// 解析错误
#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    Lex(LexError),
    UnexpectedEof,
    UnexpectedToken(String),
    ExpectedExpr,
    ExpectedRelOp,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::Lex(e) => write!(f, "lex error: {}", e),
            ParseError::UnexpectedEof => write!(f, "unexpected end of input"),
            ParseError::UnexpectedToken(s) => write!(f, "unexpected token: {}", s),
            ParseError::ExpectedExpr => write!(f, "expected expression"),
            ParseError::ExpectedRelOp => write!(f, "expected comparison operator"),
        }
    }
}

impl std::error::Error for ParseError {}

impl From<LexError> for ParseError {
    fn from(e: LexError) -> Self {
        ParseError::Lex(e)
    }
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, param_registry: &'a mut ParamRegistry) -> Self {
        let mut lexer = Lexer::new(input);
        let current = lexer.next_token().ok().flatten();
        Parser {
            lexer,
            current,
            param_registry,
        }
    }

    fn advance(&mut self) -> Result<Option<Token>, ParseError> {
        let next = self.lexer.next_token()?;
        let prev = self.current.take();
        self.current = next.clone();
        Ok(prev)
    }

    fn peek(&self) -> Option<&Token> {
        self.current.as_ref()
    }

    fn expect(&mut self, _expected: &str) -> Result<Token, ParseError> {
        let t = self.advance()?.ok_or(ParseError::UnexpectedEof)?;
        Ok(t)
    }

    fn parse_constraint_chain(&mut self) -> Result<Vec<HypothesisExpr>, ParseError> {
        let mut constraints = Vec::new();
        let mut left_expr = self.parse_expr()?;

        loop {
            let op = match self.peek() {
                Some(Token::Eq) => RelOp::Eq,
                Some(Token::LtStrict) => RelOp::Lt,
                Some(Token::Le) => RelOp::Le,
                Some(Token::GtStrict) => RelOp::Gt,
                Some(Token::Ge) => RelOp::Ge,
                Some(Token::Ne) => return Err(ParseError::UnexpectedToken("!=".into())),
                _ => break,
            };

            self.advance()?;
            let right_expr = self.parse_expr()?;

            // 若右端为常数则直接使用，否则规范化为 (left - right) op 0
            let constraint = if let Expr::Const(k) = right_expr {
                normalize_to_hypothesis_const(left_expr.clone(), op, k)
            } else {
                let diff = Expr::sub(left_expr.clone(), right_expr.clone());
                normalize_to_hypothesis_zero(diff, op)
            };
            constraints.push(constraint);
            left_expr = right_expr;
        }

        if constraints.is_empty() {
            return Err(ParseError::ExpectedRelOp);
        }

        Ok(constraints)
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        self.parse_additive()
    }

    fn parse_additive(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_multiplicative()?;

        loop {
            match self.peek() {
                Some(Token::Plus) => {
                    self.advance()?;
                    let right = self.parse_multiplicative()?;
                    left = Expr::add(left, right);
                }
                Some(Token::Minus) => {
                    self.advance()?;
                    let right = self.parse_multiplicative()?;
                    left = Expr::sub(left, right);
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_multiplicative(&mut self) -> Result<Expr, ParseError> {
        let mut left = self.parse_unary()?;

        loop {
            match self.peek() {
                Some(Token::Star) => {
                    self.advance()?;
                    let right = self.parse_unary()?;
                    // k * expr: 若 right 是 Const，则 Mul(right, left)；若 left 是 Const，则 Mul(left, right)
                    (left, _) = scalar_mul_expr(left, right)?;
                }
                Some(Token::Slash) => {
                    self.advance()?;
                    let right = self.parse_unary()?;
                    left = Expr::div(left, right);
                }
                _ => break,
            }
        }

        Ok(left)
    }

    fn parse_unary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Some(Token::Minus) => {
                self.advance()?;
                let e = self.parse_unary()?;
                Ok(Expr::mul_scalar(-1.0, e))
            }
            Some(Token::Plus) => {
                self.advance()?;
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        let t = self.advance()?.ok_or(ParseError::ExpectedExpr)?;

        match t {
            Token::Number(n) => Ok(Expr::Const(n)),
            Token::Ident(name) => {
                // 可能是参数或函数调用
                if self.peek() == Some(&Token::LParen) {
                    // 函数调用: exp(...), log(...)
                    self.advance()?;
                    let arg = self.parse_expr()?;
                    self.expect(")")?;

                    let e = match name.to_lowercase().as_str() {
                        "exp" => Expr::exp(arg),
                        "log" => Expr::log(arg),
                        _ => {
                            return Err(ParseError::UnexpectedToken(format!(
                                "unknown function: {}",
                                name
                            )));
                        }
                    };
                    Ok(e)
                } else {
                    let id = self.param_registry.get_or_insert(&name);
                    Ok(Expr::Param(id))
                }
            }
            Token::LParen => {
                let e = self.parse_expr()?;
                self.expect(")")?;
                Ok(e)
            }
            _ => Err(ParseError::UnexpectedToken(format!("{:?}", t))),
        }
    }
}

/// 尝试将 (left, right) 转为 k * expr 形式
fn scalar_mul_expr(left: Expr, right: Expr) -> Result<(Expr, bool), ParseError> {
    match (&left, &right) {
        (Expr::Const(k), _) => Ok((Expr::mul_scalar(*k, right), true)),
        (_, Expr::Const(k)) => Ok((Expr::mul_scalar(*k, left), true)),
        _ => Err(ParseError::UnexpectedToken(
            "only k * expr supported for multiplication".into(),
        )),
    }
}

/// 关系运算符
#[derive(Debug, Clone, Copy, PartialEq)]
enum RelOp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
}

/// 将 expr op k 转为 HypothesisExpr（右端为常数 k）
fn normalize_to_hypothesis_const(expr: Expr, op: RelOp, k: f64) -> HypothesisExpr {
    match op {
        RelOp::Eq => HypothesisExpr::Eq(expr, k),
        RelOp::Lt => HypothesisExpr::Lt(expr, k),
        RelOp::Le => HypothesisExpr::Le(expr, k),
        RelOp::Gt => HypothesisExpr::Gt(expr, k),
        RelOp::Ge => HypothesisExpr::Ge(expr, k),
    }
}

/// 将 (expr - right) op 0 转为 HypothesisExpr（右端为 0）
fn normalize_to_hypothesis_zero(expr: Expr, op: RelOp) -> HypothesisExpr {
    normalize_to_hypothesis_const(expr, op, 0.0)
}

/// 解析自由文本为假设约束
///
/// - 单条约束：`petal_width = 0`
/// - 链式比较：`a < b < c`
/// - 多约束（逗号分隔）：`petal_width = -0.5626, petal_length = 0.7`
pub fn parse_hypothesis(input: &str) -> Result<Vec<HypothesisExpr>, ParseError> {
    parse_hypothesis_with_registry(input, &mut ParamRegistry::new())
}

/// 使用已有 ParamRegistry 解析（用于多约束共享参数空间）
///
/// 支持逗号分隔的多条约束，如 `petal_width = -0.5626, petal_length = 0.7`
pub fn parse_hypothesis_with_registry(
    input: &str,
    param_registry: &mut ParamRegistry,
) -> Result<Vec<HypothesisExpr>, ParseError> {
    let segments: Vec<&str> = input
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    if segments.is_empty() {
        return Err(ParseError::ExpectedExpr);
    }

    let mut all_constraints = Vec::new();
    for seg in segments {
        let mut parser = Parser::new(seg, param_registry);
        let constraints = parser.parse_constraint_chain()?;
        all_constraints.extend(constraints);
    }

    Ok(all_constraints)
}
