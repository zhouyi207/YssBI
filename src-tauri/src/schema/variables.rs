//! 变量定义模块
//!
//! 定义项目中变量的完整结构，包括元数据、值配置和作用域。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ==================== 变量数据类型 ====================

/// 变量数据类型
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum VariableDataType {
    /// 整数
    Int8,
    Int16,
    Int32,
    Int64,
    Uint32,
    Uint64,
    /// 浮点数
    Float32,
    Float64,
    /// 布尔值
    Bool,
    /// 字符串
    String,
    /// 时间日期
    Date,
    Datetime,
    /// 对象
    Object,
    /// 数组 (Legacy)
    Array,
    /// 数据框 (DataFrame)
    Dataframe,
    /// 任意类型
    Any,
    /// 兼容旧版
    Int,
    Float,
}

impl Default for VariableDataType {
    fn default() -> Self {
        Self::Any
    }
}

// ==================== 变量作用域 ====================

/// 变量作用域
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum VariableScope {
    /// 全局作用域
    Global,
    /// 函数作用域
    Function {
        /// 所属函数 ID
        function_id: String,
    },
    /// 宏作用域
    Macro {
        /// 所属宏 ID
        macro_id: String,
    },
}

impl Default for VariableScope {
    fn default() -> Self {
        Self::Global
    }
}

// ==================== 数据来源配置 ====================

/// 数据来源配置（用于复杂数据类型）
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DataSourceConfig {
    /// CSV 文件
    Csv {
        /// 文件路径（相对于项目目录）
        path: String,
        /// 分隔符
        #[serde(default = "default_delimiter")]
        delimiter: String,
        /// 编码
        #[serde(default = "default_encoding")]
        encoding: String,
        /// 是否有表头
        #[serde(default = "default_true")]
        has_header: bool,
    },
    /// JSON 文件
    Json {
        /// 文件路径
        path: String,
    },
    /// Excel 文件
    Excel {
        /// 文件路径
        path: String,
        /// 工作表名称
        sheet: Option<String>,
        /// 起始行
        start_row: Option<u32>,
    },
    /// SQL 查询
    Sql {
        /// 连接标识符
        connection_id: String,
        /// SQL 查询语句
        query: String,
        /// 查询参数
        #[serde(default)]
        parameters: HashMap<String, serde_json::Value>,
    },
    /// API 请求
    Api {
        /// 请求 URL
        url: String,
        /// HTTP 方法
        #[serde(default = "default_method")]
        method: String,
        /// 请求头
        #[serde(default)]
        headers: HashMap<String, String>,
        /// 请求体
        body: Option<serde_json::Value>,
    },
    /// 数据转换（从其他变量派生）
    Transform {
        /// 源变量 ID
        source_variable_id: String,
        /// 转换操作列表
        operations: Vec<TransformOperation>,
    },
    /// 内联数据（小型数据直接存储）
    Inline {
        /// 内联数据
        data: serde_json::Value,
    },
}

/// 数据转换操作
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TransformOperation {
    /// 过滤
    Filter {
        /// 过滤表达式
        expression: String,
    },
    /// 选择列
    Select {
        /// 列名列表
        columns: Vec<String>,
    },
    /// 排序
    Sort {
        /// 排序列
        column: String,
        /// 是否降序
        #[serde(default)]
        descending: bool,
    },
    /// 分组聚合
    GroupBy {
        /// 分组列
        columns: Vec<String>,
        /// 聚合操作
        aggregations: Vec<Aggregation>,
    },
    /// 限制行数
    Limit {
        /// 行数
        count: usize,
    },
    /// 自定义表达式
    Expression {
        /// 表达式
        expr: String,
    },
}

/// 聚合操作
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Aggregation {
    /// 源列
    pub column: String,
    /// 聚合函数
    pub function: AggregateFunction,
    /// 结果别名
    pub alias: Option<String>,
}

/// 聚合函数
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateFunction {
    Sum,
    Avg,
    Min,
    Max,
    Count,
    First,
    Last,
}

// ==================== 变量定义 ====================

/// 变量定义（持久化到项目文件）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VariableDefinition {
    // ===== 元数据 =====
    /// 变量 ID
    pub id: String,
    /// 变量名称
    pub name: String,
    /// 数据类型
    pub data_type: VariableDataType,
    /// 描述
    #[serde(default)]
    pub description: String,

    // ===== 作用域 =====
    /// 变量作用域
    #[serde(default)]
    pub scope: VariableScope,

    // ===== 值配置 =====
    /// 静态初始值（简单类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_value: Option<serde_json::Value>,

    /// 数据来源配置（复杂类型）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_config: Option<DataSourceConfig>,

    // ===== 可选配置 =====
    /// 是否为数组
    #[serde(default)]
    pub is_array: bool,

    /// 是否为常量
    #[serde(default)]
    pub is_constant: bool,

    /// 默认值（执行时如果没有值则使用）
    #[serde(skip_serializing_if = "Option::is_none")]
    pub default_value: Option<serde_json::Value>,

    /// 是否暴露给外部（可被其他图引用）
    #[serde(default)]
    pub is_exposed: bool,

    /// 标签（用于分类和搜索）
    #[serde(default)]
    pub tags: Vec<String>,
}

impl VariableDefinition {
    /// 创建新的变量定义
    pub fn new(id: String, name: String, data_type: VariableDataType) -> Self {
        Self {
            id,
            name,
            data_type,
            description: String::new(),
            scope: VariableScope::default(),
            static_value: None,
            source_config: None,
            is_array: false,
            is_constant: false,
            default_value: None,
            is_exposed: false,
            tags: Vec::new(),
        }
    }

    /// 创建简单类型变量
    pub fn new_primitive(
        id: String,
        name: String,
        data_type: VariableDataType,
        value: serde_json::Value,
    ) -> Self {
        let mut var = Self::new(id, name, data_type);
        var.static_value = Some(value);
        var
    }

    /// 创建复杂类型变量
    pub fn new_complex(
        id: String,
        name: String,
        data_type: VariableDataType,
        source: DataSourceConfig,
    ) -> Self {
        let mut var = Self::new(id, name, data_type);
        var.source_config = Some(source);
        var
    }

    /// 检查是否为简单类型
    pub fn is_primitive(&self) -> bool {
        matches!(
            self.data_type,
            VariableDataType::Int
                | VariableDataType::Int8
                | VariableDataType::Int16
                | VariableDataType::Int32
                | VariableDataType::Int64
                | VariableDataType::Uint32
                | VariableDataType::Uint64
                | VariableDataType::Float
                | VariableDataType::Float32
                | VariableDataType::Float64
                | VariableDataType::Bool
                | VariableDataType::String
                | VariableDataType::Date
                | VariableDataType::Datetime
        )
    }

    /// 检查是否为复杂类型
    pub fn is_complex(&self) -> bool {
        matches!(
            self.data_type,
            VariableDataType::Dataframe | VariableDataType::Object | VariableDataType::Array
        )
    }
}

// ==================== 默认值函数 ====================

fn default_delimiter() -> String {
    ",".to_string()
}

fn default_encoding() -> String {
    "utf-8".to_string()
}

fn default_method() -> String {
    "GET".to_string()
}

fn default_true() -> bool {
    true
}

// ==================== 测试 ====================
// 测试已移动到 tests/schema_variables_tests.rs
