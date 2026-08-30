use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum MathExpr {
    Number(f64),
    Symbol(String),
    Unary {
        op: UnaryOp,
        operand: Box<MathExpr>,
    },
    Binary {
        op: BinaryOp,
        left: Box<MathExpr>,
        right: Box<MathExpr>,
    },
    Call {
        name: String,
        args: Vec<MathExpr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Pow,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MathRelation {
    pub left: MathExpr,
    pub op: ComparisonOp,
    pub right: MathExpr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComparisonOp {
    Eq,
    Lt,
    Le,
    Gt,
    Ge,
    DistributedAs,
}

impl ComparisonOp {
    pub fn reversed(self) -> Self {
        match self {
            Self::Eq => Self::Eq,
            Self::Lt => Self::Gt,
            Self::Le => Self::Ge,
            Self::Gt => Self::Lt,
            Self::Ge => Self::Le,
            Self::DistributedAs => Self::DistributedAs,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MathErrorKind {
    EmptyInput,
    Parse,
    MissingRelation,
    Unsupported,
    UnknownFunction,
    AmbiguousSymbol,
    NonFiniteNumber,
    InputLimit,
    RelationLimit,
    NodeLimit,
    DepthLimit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MathError {
    pub kind: MathErrorKind,
    pub message: String,
    pub offset: Option<usize>,
}

impl MathError {
    pub(crate) fn new(kind: MathErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
            offset: None,
        }
    }

    pub(crate) fn at(mut self, offset: usize) -> Self {
        self.offset = Some(offset);
        self
    }
}

impl fmt::Display for MathError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(offset) = self.offset {
            write!(f, "{}（位置 {}）", self.message, offset)
        } else {
            f.write_str(&self.message)
        }
    }
}

impl std::error::Error for MathError {}
