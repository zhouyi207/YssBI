use super::DataValue;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;

/// 基础值类型
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "inner")]
pub enum DataType {
    // 基础类型
    Boolean,

    // 为数据库展示预留的类型
    Int32,
    Float32,

    // 系统核心类型
    Int64,
    Float64,
    String,

    /// 日期类型（Polars Date / Datetime 统一映射）
    Date,

    /// 分类类型（Polars Categorical / Enum）
    Categorical,

    // 复合类型
    Array(Box<DataType>),
    Object,

    // 数据框架
    DataFrame,
    DataSeries(Box<DataType>),

    /// 用户定义的不透明结构体类型（句柄传递）
    ///
    /// type key 标识具体类型（如 "StandardizeTransform1D"），
    /// 运行时值为句柄 ID，实际对象存于 ExecutionDataStore。
    Struct(std::string::String),

    /// 联合类型：接受多种类型之一（如 OLS Exog 同时接受 Float64 和 String）
    OneOf(Vec<DataType>),

    // 特殊类型
    Any,
}

impl DataType {
    /// 构造 OneOf，自动展平嵌套并去重；单元素退化为该元素本身，含 Any 退化为 Any
    pub fn one_of(types: Vec<DataType>) -> DataType {
        let mut flat: Vec<DataType> = Vec::new();
        for t in types {
            match t {
                DataType::Any => return DataType::Any,
                DataType::OneOf(inner) => {
                    for it in inner {
                        if it == DataType::Any {
                            return DataType::Any;
                        }
                        if !flat.contains(&it) {
                            flat.push(it);
                        }
                    }
                }
                other => {
                    if !flat.contains(&other) {
                        flat.push(other);
                    }
                }
            }
        }
        match flat.len() {
            0 => DataType::Any,
            1 => flat.into_iter().next().unwrap(),
            _ => DataType::OneOf(flat),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::DataType;
    use crate::graph::value::{StructTypeMeta, TypeSystemSnapshot};
    use std::collections::BTreeMap;

    fn model_type_system() -> TypeSystemSnapshot {
        let mut struct_types = BTreeMap::new();
        struct_types.insert(
            "Model".to_string(),
            StructTypeMeta {
                key: "Model".to_string(),
                parents: vec![],
                category: Some("model".to_string()),
                display_name: Some("Model".to_string()),
            },
        );
        struct_types.insert(
            "OLSModel".to_string(),
            StructTypeMeta {
                key: "OLSModel".to_string(),
                parents: vec!["Model".to_string()],
                category: Some("model".to_string()),
                display_name: Some("OLS Model".to_string()),
            },
        );
        TypeSystemSnapshot { struct_types }
    }

    #[test]
    fn data_type_struct_acceptance_is_exact_without_type_system() {
        let target = DataType::Struct("Model".to_string());
        let source = DataType::Struct("OLSModel".to_string());

        assert!(!target.can_accept(&source));
    }

    #[test]
    fn type_system_accepts_concrete_ols_model_for_model_family() {
        let type_system = model_type_system();
        let target = DataType::Struct("Model".to_string());
        let source = DataType::Struct("OLSModel".to_string());

        assert!(type_system.can_accept(&target, &source));
    }

    #[test]
    fn type_system_rejects_unrelated_structs_for_model_family() {
        let type_system = model_type_system();
        let target = DataType::Struct("Model".to_string());
        let source = DataType::Struct("OLSResult".to_string());

        assert!(!type_system.can_accept(&target, &source));
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataType::Boolean => write!(f, "Boolean"),
            DataType::Int32 => write!(f, "Int32"),
            DataType::Int64 => write!(f, "Int64"),
            DataType::Float32 => write!(f, "Float32"),
            DataType::Float64 => write!(f, "Float64"),
            DataType::String => write!(f, "String"),
            DataType::Date => write!(f, "Date"),
            DataType::Categorical => write!(f, "Categorical"),
            DataType::Array(inner) => write!(f, "Array<{}>", inner),
            DataType::Object => write!(f, "Object"),
            DataType::DataFrame => write!(f, "DataFrame"),
            DataType::DataSeries(inner) => write!(f, "DataSeries<{}>", inner),
            DataType::Struct(key) => write!(f, "Struct<{}>", key),
            DataType::OneOf(types) => {
                for (i, t) in types.iter().enumerate() {
                    if i > 0 {
                        write!(f, " | ")?;
                    }
                    write!(f, "{}", t)?;
                }
                Ok(())
            }
            DataType::Any => write!(f, "Any"),
        }
    }
}

impl FromStr for DataType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();

        // 先检查是否是 `|` 分隔的联合类型（只在顶层 `<>` 外的 `|` 处拆分）
        let parts = split_top_level(trimmed, '|');
        if parts.len() > 1 {
            let types: Result<Vec<DataType>, String> = parts.iter().map(|p| p.parse()).collect();
            return Ok(DataType::one_of(types?));
        }

        match trimmed {
            "Boolean" => Ok(DataType::Boolean),
            "Int32" => Ok(DataType::Int32),
            "Int64" => Ok(DataType::Int64),
            "Float32" => Ok(DataType::Float32),
            "Float64" => Ok(DataType::Float64),
            "String" => Ok(DataType::String),
            "Date" => Ok(DataType::Date),
            "Categorical" => Ok(DataType::Categorical),
            "Object" => Ok(DataType::Object),
            "DataFrame" => Ok(DataType::DataFrame),
            "DataSeries" => Ok(DataType::DataSeries(Box::new(DataType::Any))),
            "Any" => Ok(DataType::Any),
            _ => {
                if let Some(inner) = trimmed
                    .strip_prefix("Array<")
                    .and_then(|s| s.strip_suffix('>'))
                {
                    let inner = inner.parse()?;
                    return Ok(DataType::Array(Box::new(inner)));
                }
                if let Some(inner) = trimmed
                    .strip_prefix("DataSeries<")
                    .and_then(|s| s.strip_suffix('>'))
                {
                    let inner = inner.parse()?;
                    return Ok(DataType::DataSeries(Box::new(inner)));
                }
                if let Some(key) = trimmed
                    .strip_prefix("Struct<")
                    .and_then(|s| s.strip_suffix('>'))
                {
                    return Ok(DataType::Struct(key.to_string()));
                }
                Err(format!("Unknown DataType: {}", trimmed))
            }
        }
    }
}

/// 在顶层（不在 `<>` 内部）按分隔符拆分字符串
fn split_top_level(s: &str, sep: char) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0usize;
    let mut start = 0;
    for (i, c) in s.char_indices() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            c if c == sep && depth == 0 => {
                parts.push(s[start..i].trim());
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    let tail = s[start..].trim();
    if !tail.is_empty() {
        parts.push(tail);
    }
    parts
}

impl DataType {
    /// 返回该类型的默认值（用于 Pin 占位、变量初始化等）
    pub fn default_value(&self) -> DataValue {
        match self {
            DataType::Boolean => DataValue::Boolean(false),
            DataType::Int32 => DataValue::Int32(0),
            DataType::Int64 => DataValue::Int64(0),
            DataType::Float32 => DataValue::Float32(num_traits::Zero::zero()),
            DataType::Float64 => DataValue::Float64(num_traits::Zero::zero()),
            DataType::String => DataValue::String(String::new()),
            DataType::Date => DataValue::String(String::new()), // 默认空日期，用字符串表示
            DataType::Categorical => DataValue::String(String::new()), // 分类默认空字符串
            DataType::Array(_) => DataValue::Array(Vec::new()),
            DataType::Object => DataValue::Object(std::collections::HashMap::new()),
            DataType::OneOf(types) => types.first().map_or(DataValue::Null, |t| t.default_value()),
            DataType::Any | DataType::DataFrame | DataType::DataSeries(_) | DataType::Struct(_) => {
                DataValue::Null
            }
        }
    }

    /// 是否为标量/基础类型（非复合、非 Any）
    pub fn is_primitive(&self) -> bool {
        match self {
            DataType::Boolean
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
            | DataType::String
            | DataType::Date
            | DataType::Categorical => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(|t| t.is_primitive()),
            _ => false,
        }
    }

    /// 是否为数值类型
    pub fn is_numeric(&self) -> bool {
        match self {
            DataType::Int32 | DataType::Int64 | DataType::Float32 | DataType::Float64 => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(|t| t.is_numeric()),
            _ => false,
        }
    }

    /// 是否支持比较运算（==, !=, <, > 等）
    pub fn is_comparable(&self) -> bool {
        match self {
            DataType::Boolean
            | DataType::Int32
            | DataType::Int64
            | DataType::Float32
            | DataType::Float64
            | DataType::String
            | DataType::Date
            | DataType::Categorical => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(|t| t.is_comparable()),
            _ => false,
        }
    }

    /// 是否可迭代（for-in、map 等）
    pub fn is_iterable(&self) -> bool {
        match self {
            DataType::Array(_) | DataType::String | DataType::DataSeries(_) => true,
            DataType::OneOf(types) => !types.is_empty() && types.iter().all(|t| t.is_iterable()),
            _ => false,
        }
    }

    /// Array 的元素类型，非 Array 返回 None
    pub fn array_inner(&self) -> Option<&DataType> {
        match self {
            DataType::Array(inner) => Some(inner),
            _ => None,
        }
    }

    /// DataSeries 的元素类型，非 DataSeries 返回 None
    pub fn series_inner(&self) -> Option<&DataType> {
        match self {
            DataType::DataSeries(inner) => Some(inner),
            _ => None,
        }
    }

    /// Convert 节点使用：判断 from 能否通过类型转换变为 to
    pub fn can_convert(from: &DataType, to: &DataType) -> bool {
        if from == to {
            return true;
        }
        match (from, to) {
            (_, DataType::Any) => true,
            (_, DataType::String) => true,
            // OneOf 目标：from 能转为任一成员即可
            (_, DataType::OneOf(targets)) => targets.iter().any(|t| DataType::can_convert(from, t)),
            // OneOf 源：任一成员能转为 to 即可
            (DataType::OneOf(sources), _) => sources.iter().any(|s| DataType::can_convert(s, to)),
            (
                _,
                DataType::Boolean
                | DataType::Int32
                | DataType::Int64
                | DataType::Float32
                | DataType::Float64
                | DataType::Date
                | DataType::Categorical,
            ) => from.is_primitive(),
            _ => false,
        }
    }

    /// 检查 from 类型的值是否可以赋给本类型
    pub fn can_accept(&self, from: &DataType) -> bool {
        if from == self {
            return true;
        }
        if matches!(self, DataType::Any) || matches!(from, DataType::Any) {
            return true;
        }
        match (from, self) {
            // OneOf 在目标端：from 匹配任一成员即可
            (_, DataType::OneOf(targets)) => targets.iter().any(|t| t.can_accept(from)),
            // OneOf 在源端：任一成员能被目标接受即可（宽松策略，配合右键收窄使用）
            (DataType::OneOf(sources), _) => sources.iter().any(|s| self.can_accept(s)),
            // 容器类型：内层递归（自然支持 DataSeries<OneOf(...)>）
            (DataType::Array(from_inner), DataType::Array(to_inner)) => {
                to_inner.can_accept(from_inner)
            }
            (DataType::DataSeries(from_inner), DataType::DataSeries(to_inner)) => {
                to_inner.can_accept(from_inner)
            }
            (DataType::Struct(from_key), DataType::Struct(to_key)) => from_key == to_key,
            _ => false,
        }
    }
}
