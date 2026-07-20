use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use super::model::{BinaryOp, MathFunction, UnaryOp};

const MAX_EXPRESSION_NODES: usize = 256;
const MAX_EXPRESSION_DEPTH: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RawExpression {
    Number {
        value: f64,
    },
    Symbol {
        name: String,
    },
    Unary {
        op: UnaryOp,
        arg: Box<RawExpression>,
    },
    Binary {
        op: BinaryOp,
        left: Box<RawExpression>,
        right: Box<RawExpression>,
    },
    Call {
        function: MathFunction,
        args: Vec<RawExpression>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ParsedExpression {
    pub formula_text: String,
    pub response_symbol: Option<String>,
    pub raw_predictor: RawExpression,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpressionParseError {
    pub message: String,
}

impl ExpressionParseError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for ExpressionParseError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ExpressionParseError {}

#[derive(Debug, Clone, PartialEq)]
enum Token {
    Number(f64),
    Identifier(String),
    Operator(char),
    LeftParen,
    RightParen,
    Comma,
    Eof,
}

pub fn parse_model_expression(input: &str) -> Result<ParsedExpression, ExpressionParseError> {
    let normalized = normalize_latex_expression(input);
    let (response_symbol, predictor_text) = split_model_expression(&normalized);
    let raw_predictor = parse_predictor_expression(predictor_text)?;
    validate_expression_limits(&raw_predictor)?;
    let mut symbols = BTreeSet::new();
    collect_raw_symbols(&raw_predictor, &mut symbols);
    if let Some(response) = &response_symbol {
        symbols.insert(response.clone());
    }

    Ok(ParsedExpression {
        formula_text: input.trim().to_string(),
        response_symbol,
        raw_predictor,
        symbols: symbols.into_iter().collect(),
    })
}

pub fn parse_predictor_expression(input: &str) -> Result<RawExpression, ExpressionParseError> {
    let normalized = normalize_latex_expression(input);
    let mut parser = Parser::new(tokenize(&normalized)?);
    let expression = parser.parse_expression()?;
    parser.expect_end()?;
    validate_expression_limits(&expression)?;
    Ok(expression)
}

pub fn collect_raw_symbols(expression: &RawExpression, symbols: &mut BTreeSet<String>) {
    match expression {
        RawExpression::Number { .. } => {}
        RawExpression::Symbol { name } => {
            symbols.insert(name.clone());
        }
        RawExpression::Unary { arg, .. } => collect_raw_symbols(arg, symbols),
        RawExpression::Binary { left, right, .. } => {
            collect_raw_symbols(left, symbols);
            collect_raw_symbols(right, symbols);
        }
        RawExpression::Call { args, .. } => {
            for arg in args {
                collect_raw_symbols(arg, symbols);
            }
        }
    }
}

fn normalize_latex_expression(input: &str) -> String {
    input
        .replace("\\cdot", "*")
        .replace("\\times", "*")
        .replace("\\sigma", "sigma")
        .replace("\\left", "")
        .replace("\\right", "")
        .replace("\\sim", "~")
        .replace("−", "-")
        .trim()
        .to_string()
}

fn split_model_expression(input: &str) -> (Option<String>, &str) {
    if let Some((left, right)) = input.split_once('=') {
        return (parse_response_symbol(left), right.trim());
    }
    if let Some((left, right)) = input.split_once('~') {
        let response = parse_response_symbol(left);
        return (
            response,
            extract_first_distribution_arg(right).unwrap_or(right.trim()),
        );
    }
    (None, input.trim())
}

fn parse_response_symbol(input: &str) -> Option<String> {
    let candidate = input.trim();
    if is_valid_symbol(candidate) {
        Some(candidate.to_string())
    } else {
        None
    }
}

fn extract_first_distribution_arg(input: &str) -> Option<&str> {
    let start = input.find('(')?;
    let end = input.rfind(')')?;
    let args = &input[start + 1..end];
    let mut depth = 0usize;
    for (index, character) in args.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            ',' if depth == 0 => return Some(args[..index].trim()),
            _ => {}
        }
    }
    Some(args.trim())
}

fn tokenize(input: &str) -> Result<Vec<Token>, ExpressionParseError> {
    let chars: Vec<char> = input.chars().collect();
    let mut tokens = Vec::new();
    let mut index = 0usize;

    while index < chars.len() {
        let character = chars[index];
        if character.is_whitespace() {
            index += 1;
            continue;
        }

        if character.is_ascii_digit() || character == '.' {
            let start = index;
            index += 1;
            while index < chars.len() {
                let current = chars[index];
                let previous = chars[index - 1];
                if current == '+' || current == '-' {
                    if previous != 'e' && previous != 'E' {
                        break;
                    }
                } else if !current.is_ascii_digit()
                    && current != '.'
                    && current != 'e'
                    && current != 'E'
                {
                    break;
                }
                index += 1;
            }
            let raw: String = chars[start..index].iter().collect();
            let value = raw
                .parse::<f64>()
                .map_err(|_| ExpressionParseError::new(format!("Invalid number near {raw}")))?;
            if !value.is_finite() {
                return Err(ExpressionParseError::new(format!(
                    "Invalid finite number near {raw}"
                )));
            }
            tokens.push(Token::Number(value));
            continue;
        }

        if character.is_ascii_alphabetic() || character == '_' {
            let start = index;
            index += 1;
            while index < chars.len()
                && (chars[index].is_ascii_alphanumeric() || chars[index] == '_')
            {
                index += 1;
            }
            let identifier: String = chars[start..index].iter().collect();
            if !is_valid_symbol(&identifier) {
                return Err(ExpressionParseError::new(format!(
                    "Invalid identifier {identifier}"
                )));
            }
            tokens.push(Token::Identifier(identifier));
            continue;
        }

        match character {
            '+' | '-' | '*' | '/' | '^' => tokens.push(Token::Operator(character)),
            '(' => tokens.push(Token::LeftParen),
            ')' => tokens.push(Token::RightParen),
            ',' => tokens.push(Token::Comma),
            _ => {
                return Err(ExpressionParseError::new(format!(
                    "Unsupported character {character}"
                )));
            }
        }
        index += 1;
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

fn is_valid_symbol(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_alphabetic() || first == '_')
        && chars.all(|character| character.is_ascii_alphanumeric() || character == '_')
}

fn validate_expression_limits(expression: &RawExpression) -> Result<(), ExpressionParseError> {
    let node_count = count_nodes(expression);
    if node_count > MAX_EXPRESSION_NODES {
        return Err(ExpressionParseError::new(format!(
            "Expression is too large: {node_count} nodes"
        )));
    }
    let depth = expression_depth(expression);
    if depth > MAX_EXPRESSION_DEPTH {
        return Err(ExpressionParseError::new(format!(
            "Expression is too deep: depth {depth}"
        )));
    }
    Ok(())
}

fn count_nodes(expression: &RawExpression) -> usize {
    match expression {
        RawExpression::Number { .. } | RawExpression::Symbol { .. } => 1,
        RawExpression::Unary { arg, .. } => 1 + count_nodes(arg),
        RawExpression::Binary { left, right, .. } => 1 + count_nodes(left) + count_nodes(right),
        RawExpression::Call { args, .. } => 1 + args.iter().map(count_nodes).sum::<usize>(),
    }
}

fn expression_depth(expression: &RawExpression) -> usize {
    match expression {
        RawExpression::Number { .. } | RawExpression::Symbol { .. } => 1,
        RawExpression::Unary { arg, .. } => 1 + expression_depth(arg),
        RawExpression::Binary { left, right, .. } => {
            1 + expression_depth(left).max(expression_depth(right))
        }
        RawExpression::Call { args, .. } => {
            1 + args.iter().map(expression_depth).max().unwrap_or(0)
        }
    }
}

struct Parser {
    tokens: Vec<Token>,
    position: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            position: 0,
        }
    }

    fn parse_expression(&mut self) -> Result<RawExpression, ExpressionParseError> {
        self.parse_add_sub()
    }

    fn expect_end(&self) -> Result<(), ExpressionParseError> {
        match self.current() {
            Token::Eof => Ok(()),
            token => Err(ExpressionParseError::new(format!(
                "Unexpected trailing token {token:?}"
            ))),
        }
    }

    fn parse_add_sub(&mut self) -> Result<RawExpression, ExpressionParseError> {
        let mut left = self.parse_mul_div()?;
        loop {
            let op = match self.current() {
                Token::Operator('+') => BinaryOp::Add,
                Token::Operator('-') => BinaryOp::Sub,
                _ => break,
            };
            self.advance();
            let right = self.parse_mul_div()?;
            left = RawExpression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_mul_div(&mut self) -> Result<RawExpression, ExpressionParseError> {
        let mut left = self.parse_power()?;
        loop {
            let op = match self.current() {
                Token::Operator('*') => BinaryOp::Mul,
                Token::Operator('/') => BinaryOp::Div,
                _ => break,
            };
            self.advance();
            let right = self.parse_power()?;
            left = RawExpression::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn parse_power(&mut self) -> Result<RawExpression, ExpressionParseError> {
        let left = self.parse_unary()?;
        if !matches!(self.current(), Token::Operator('^')) {
            return Ok(left);
        }
        self.advance();
        let right = self.parse_power()?;
        Ok(RawExpression::Binary {
            op: BinaryOp::Pow,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    fn parse_unary(&mut self) -> Result<RawExpression, ExpressionParseError> {
        match self.current() {
            Token::Operator('-') => {
                self.advance();
                Ok(RawExpression::Unary {
                    op: UnaryOp::Neg,
                    arg: Box::new(self.parse_unary()?),
                })
            }
            Token::Operator('+') => {
                self.advance();
                self.parse_unary()
            }
            _ => self.parse_primary(),
        }
    }

    fn parse_primary(&mut self) -> Result<RawExpression, ExpressionParseError> {
        match self.current().clone() {
            Token::Number(value) => {
                self.advance();
                Ok(RawExpression::Number { value })
            }
            Token::Identifier(identifier) => {
                self.advance();
                if matches!(self.current(), Token::LeftParen) {
                    return self.parse_call(identifier);
                }
                Ok(RawExpression::Symbol { name: identifier })
            }
            Token::LeftParen => {
                self.advance();
                let expression = self.parse_expression()?;
                if !matches!(self.current(), Token::RightParen) {
                    return Err(ExpressionParseError::new("Expected closing parenthesis"));
                }
                self.advance();
                Ok(expression)
            }
            token => Err(ExpressionParseError::new(format!(
                "Expected expression, got {token:?}"
            ))),
        }
    }

    fn parse_call(&mut self, identifier: String) -> Result<RawExpression, ExpressionParseError> {
        let function = parse_math_function(&identifier)?;
        self.advance();
        let mut args = Vec::new();
        if !matches!(self.current(), Token::RightParen) {
            loop {
                args.push(self.parse_expression()?);
                if !matches!(self.current(), Token::Comma) {
                    break;
                }
                self.advance();
            }
        }
        if !matches!(self.current(), Token::RightParen) {
            return Err(ExpressionParseError::new(format!(
                "Expected closing parenthesis for {identifier}"
            )));
        }
        self.advance();
        Ok(RawExpression::Call { function, args })
    }

    fn current(&self) -> &Token {
        self.tokens.get(self.position).unwrap_or(&Token::Eof)
    }

    fn advance(&mut self) {
        if !matches!(self.current(), Token::Eof) {
            self.position += 1;
        }
    }
}

fn parse_math_function(identifier: &str) -> Result<MathFunction, ExpressionParseError> {
    match identifier {
        "exp" => Ok(MathFunction::Exp),
        "log" => Ok(MathFunction::Log),
        "sqrt" => Ok(MathFunction::Sqrt),
        "abs" => Ok(MathFunction::Abs),
        "sin" => Ok(MathFunction::Sin),
        "cos" => Ok(MathFunction::Cos),
        "min" => Ok(MathFunction::Min),
        "max" => Ok(MathFunction::Max),
        _ => Err(ExpressionParseError::new(format!(
            "Unsupported function {identifier}"
        ))),
    }
}
