use crate::graph::DataType;
use serde::{Deserialize, Serialize};

/// Pin Schema（用于描述复杂类型的结构）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PinSchema {
    /// DataFrame 的列结构
    DataFrame(DataFrameSchema),
    // 未来可以扩展：
    // Struct(StructSchema),
    // Enum(EnumSchema),
}

/// DataFrame 的 Schema 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataFrameSchema {
    /// 列定义
    pub columns: Vec<ColumnSchema>,
}

impl DataFrameSchema {
    /// 创建新的 DataFrame Schema
    pub fn new(columns: Vec<ColumnSchema>) -> Self {
        Self { columns }
    }

    /// 获取列的数量
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// 通过名称查找列
    pub fn find_column(&self, name: &str) -> Option<&ColumnSchema> {
        self.columns.iter().find(|col| col.name == name)
    }

    /// 获取列名列表
    pub fn column_names(&self) -> Vec<&str> {
        self.columns.iter().map(|col| col.name.as_str()).collect()
    }

    /// 检查是否包含指定列
    pub fn has_column(&self, name: &str) -> bool {
        self.columns.iter().any(|col| col.name == name)
    }
}

/// 列的 Schema 定义
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ColumnSchema {
    /// 列名
    pub name: String,
    
    /// 列的数据类型
    pub ty: DataType,
    
    /// 是否可为空
    pub nullable: bool,
}

impl ColumnSchema {
    /// 创建新的列 Schema
    pub fn new(name: impl Into<String>, ty: DataType) -> Self {
        Self {
            name: name.into(),
            ty,
            nullable: false,
        }
    }

    /// 设置为可空
    pub fn nullable(mut self) -> Self {
        self.nullable = true;
        self
    }
}
