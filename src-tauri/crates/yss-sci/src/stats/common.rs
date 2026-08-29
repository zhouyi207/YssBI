//! 假设检验公共类型

/// 备择假设类型（t 检验、Wald 检验共用）
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alternative {
    TwoSided,
    Greater,
    Less,
}
