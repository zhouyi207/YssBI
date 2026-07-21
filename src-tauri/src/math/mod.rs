mod adapter;
mod ir;

pub use ir::{BinaryOp, ComparisonOp, MathError, MathErrorKind, MathExpr, MathRelation, UnaryOp};

pub const MAX_RELATIONS: usize = 64;

pub fn ensure_relation_count(count: usize) -> Result<(), MathError> {
    adapter::ensure_relation_count(count)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathInputFormat {
    Plain,
    Latex,
}

#[derive(Debug, Clone, Copy)]
pub struct ParseOptions<'a> {
    pub format: MathInputFormat,
    pub known_symbols: &'a [String],
}

impl<'a> ParseOptions<'a> {
    pub fn plain(known_symbols: &'a [String]) -> Self {
        Self {
            format: MathInputFormat::Plain,
            known_symbols,
        }
    }

    pub fn latex(known_symbols: &'a [String]) -> Self {
        Self {
            format: MathInputFormat::Latex,
            known_symbols,
        }
    }
}

pub fn parse_expression(input: &str, options: ParseOptions<'_>) -> Result<MathExpr, MathError> {
    adapter::parse_expression(input, options)
}

pub fn parse_relations(
    input: &str,
    options: ParseOptions<'_>,
) -> Result<Vec<MathRelation>, MathError> {
    adapter::parse_relations(input, options)
}
