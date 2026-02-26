//! 数据值表示

use super::DataType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::{Add, Sub, Mul, Div};

/// 分类变量的语义角色
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CategoricalRole {
    /// 普通分类变量（如性别、地区）
    General,
    /// 个体/实体标识（面板数据固定效应）
    Individual,
    /// 时间周期标识（面板数据固定效应，公式中用 t 表示）
    Time,
}

/// 哑变量编码元信息（附加在 String 类型 DataSeries 上，供 OLS 等节点消费）
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DummyInfo {
    /// 要剔除的参考类别；None 表示剔除第一个 unique 值
    pub drop_category: Option<String>,
    /// 语义角色
    pub role: CategoricalRole,
}

/// DataSeries 值（ID + 可选的元素类型 + 可选的哑变量元信息）
#[derive(Debug, Clone, PartialEq)]
pub struct DataSeriesValue {
    pub id: String,
    pub element_type: Option<DataType>,
    pub dummy_info: Option<DummyInfo>,
}

impl DataSeriesValue {
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            element_type: None,
            dummy_info: None,
        }
    }

    pub fn with_element_type(id: impl Into<String>, element_type: DataType) -> Self {
        Self {
            id: id.into(),
            element_type: Some(element_type),
            dummy_info: None,
        }
    }

    pub fn with_dummy_info(mut self, dummy_info: DummyInfo) -> Self {
        self.dummy_info = Some(dummy_info);
        self
    }
}

impl Serialize for DataSeriesValue {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        if self.element_type.is_none() && self.dummy_info.is_none() {
            serializer.serialize_str(&self.id)
        } else {
            use serde::ser::SerializeStruct;
            let field_count = 1
                + self.element_type.is_some() as usize
                + self.dummy_info.is_some() as usize;
            let mut s = serializer.serialize_struct("DataSeries", field_count)?;
            s.serialize_field("id", &self.id)?;
            if self.element_type.is_some() {
                s.serialize_field("elementType", &self.element_type)?;
            }
            if self.dummy_info.is_some() {
                s.serialize_field("dummyInfo", &self.dummy_info)?;
            }
            s.end()
        }
    }
}

impl<'de> Deserialize<'de> for DataSeriesValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Payload {
            IdOnly(String),
            Full {
                id: String,
                #[serde(rename = "elementType")]
                element_type: Option<DataType>,
                #[serde(rename = "dummyInfo")]
                dummy_info: Option<DummyInfo>,
            },
        }
        let p = Payload::deserialize(deserializer)?;
        match p {
            Payload::IdOnly(id) => Ok(DataSeriesValue {
                id,
                element_type: None,
                dummy_info: None,
            }),
            Payload::Full { id, element_type, dummy_info } => Ok(DataSeriesValue {
                id,
                element_type,
                dummy_info,
            }),
        }
    }
}

/// 运行时数据值
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum DataValue {
    // 基础类型
    Boolean(bool),
    Int32(i32),
    Int64(i64),
    Float32(f32),
    Float64(f64),
    String(String),

    // 复合类型
    Array(Vec<DataValue>),
    Object(HashMap<String, DataValue>),
    DataFrame(String),   // DataFrame ID
    DataSeries(DataSeriesValue),  // DataSeries ID + 可选元素类型

    /// 用户定义结构体的不透明句柄
    ///
    /// `type_key`: 类型标识（如 "StandardizeTransform1D"）
    /// `handle_id`: 执行期数据缓存中的引用 ID
    Struct {
        #[serde(rename = "typeKey")]
        type_key: String,
        #[serde(rename = "handleId")]
        handle_id: String,
    },

    Null,
}

impl DataValue {
    /// 获取值的类型
    pub fn value_type(&self) -> Option<DataType> {
        match self {
            DataValue::Boolean(_) => Some(DataType::Boolean),
            DataValue::Int32(_) => Some(DataType::Int32),
            DataValue::Int64(_) => Some(DataType::Int64),
            DataValue::Float32(_) => Some(DataType::Float32),
            DataValue::Float64(_) => Some(DataType::Float64),
            DataValue::String(_) => Some(DataType::String),
            DataValue::Array(arr) => {
                if let Some(first) = arr.first() {
                    Some(DataType::Array(Box::new(first.value_type().unwrap())))
                } else {
                    Some(DataType::Array(Box::new(DataType::Any)))
                }
            }
            DataValue::Object(_) => Some(DataType::Object),
            DataValue::Null => None,
            DataValue::Struct { type_key, .. } => Some(DataType::Struct(type_key.clone())),
            DataValue::DataFrame(_) => Some(DataType::DataFrame),
            DataValue::DataSeries(v) => Some(DataType::DataSeries(Box::new(
                v.element_type.clone().unwrap_or(DataType::Any),
            ))),
        }
    }

    // 类型转换辅助方法
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            DataValue::Boolean(b) => Some(*b),
            DataValue::Int32(n) => Some(*n != 0),
            DataValue::Int64(n) => Some(*n != 0),
            DataValue::Float32(n) => Some(*n != 0.0),
            DataValue::Float64(n) => Some(*n != 0.0),
            DataValue::String(s) => Some(!s.is_empty()),
            DataValue::Null => Some(false),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            DataValue::Int32(i) => Some(*i),
            DataValue::Int64(i) => Some(*i as i32),
            DataValue::Float32(f) => Some(*f as i32),
            DataValue::Float64(f) => Some(*f as i32),
            DataValue::Boolean(b) => Some(if *b { 1 } else { 0 }),
            _ => None,
        }
    }

    pub fn as_i64(&self) -> Option<i64> {
        match self {
            DataValue::Int64(i) => Some(*i),
            DataValue::Int32(i) => Some(*i as i64),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<f32> {
        match self {
            DataValue::Float32(f) => Some(*f),
            DataValue::Float64(f) => Some(*f as f32),
            DataValue::Int32(i) => Some(*i as f32),
            DataValue::Int64(i) => Some(*i as f32),
            DataValue::Boolean(b) => Some(if *b { 1.0 } else { 0.0 }),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            DataValue::Float64(f) => Some(*f),
            DataValue::Float32(f) => Some(*f as f64),
            DataValue::Int64(i) => Some(*i as f64),
            DataValue::Int32(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// 创建一个 Struct 句柄值
    pub fn new_struct(type_key: impl Into<String>, handle_id: impl Into<String>) -> Self {
        DataValue::Struct {
            type_key: type_key.into(),
            handle_id: handle_id.into(),
        }
    }

    /// 获取 Struct 句柄 ID（仅 Struct 变体）
    pub fn as_handle_id(&self) -> Option<&str> {
        match self {
            DataValue::Struct { handle_id, .. } => Some(handle_id),
            _ => None,
        }
    }

    pub fn as_string(&self) -> Option<&str> {
        match self {
            DataValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// 将值强制转换为目标类型。
    /// 如果转换失败（类型不兼容），返回原值不变。
    pub fn coerce_to(&self, target: &DataType) -> DataValue {
        if let Some(my_type) = self.value_type() {
            if my_type == *target {
                return self.clone();
            }
        }

        match target {
            DataType::Boolean => self
                .as_bool()
                .map(DataValue::Boolean)
                .unwrap_or_else(|| self.clone()),
            DataType::Int32 => self
                .as_i32()
                .map(DataValue::Int32)
                .unwrap_or_else(|| self.clone()),
            DataType::Int64 => self
                .as_i64()
                .map(DataValue::Int64)
                .unwrap_or_else(|| self.clone()),
            DataType::Float32 => self
                .as_f32()
                .map(DataValue::Float32)
                .unwrap_or_else(|| self.clone()),
            DataType::Float64 => self
                .as_f64()
                .map(DataValue::Float64)
                .unwrap_or_else(|| self.clone()),
            DataType::String => {
                let s = match self {
                    DataValue::String(s) => return DataValue::String(s.clone()),
                    DataValue::Boolean(b) => b.to_string(),
                    DataValue::Int32(n) => n.to_string(),
                    DataValue::Int64(n) => n.to_string(),
                    DataValue::Float32(n) => n.to_string(),
                    DataValue::Float64(n) => n.to_string(),
                    DataValue::Null => "null".to_string(),
                    DataValue::DataFrame(id) => format!("DataFrame({})", id),
                    DataValue::DataSeries(v) => format!("DataSeries({})", v.id),
                    DataValue::Struct { type_key, handle_id } => format!("Struct<{}>({})", type_key, handle_id),
                    _ => return self.clone(),
                };
                DataValue::String(s)
            }
            DataType::Any => self.clone(),
            DataType::DataFrame => match self {
                DataValue::DataFrame(_) => self.clone(),
                _ => self.clone(),
            },
            DataType::DataSeries(_) => match self {
                DataValue::DataSeries(_) => self.clone(), // 引用类型，透传
                _ => self.clone(),
            },
            DataType::Array(target_inner) => match self {
                DataValue::Array(arr) => DataValue::Array(
                    arr.iter()
                        .map(|v| v.coerce_to(target_inner))
                        .collect(),
                ),
                _ => self.clone(),
            },
            DataType::Object => match self {
                DataValue::Object(_) => self.clone(),
                _ => self.clone(),
            },
            DataType::Struct(_) => self.clone(),
            DataType::OneOf(_) => self.clone(),
        }
    }
}

impl Default for DataValue {
    fn default() -> Self {
        DataValue::Null
    }
}

// ============================================================================
// 运算符重载实现
// ============================================================================

/// 加法运算符实现
impl Add for DataValue {
    type Output = Result<DataValue, String>;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 仅同类型运算，类型转换需使用 convert 节点
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a + b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a + b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a + b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a + b)),
            (DataValue::String(a), DataValue::String(b)) => Ok(DataValue::String(format!("{}{}", a, b))),
            
            (a, b) => Err(format!(
                "Cannot add {:?} and {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

/// 减法运算符实现
impl Sub for DataValue {
    type Output = Result<DataValue, String>;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 仅同类型运算，类型转换需使用 convert 节点
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a - b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a - b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a - b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a - b)),
            
            (a, b) => Err(format!(
                "Cannot subtract {:?} from {:?}: incompatible types",
                b.value_type(),
                a.value_type()
            )),
        }
    }
}

/// 乘法运算符实现
impl Mul for DataValue {
    type Output = Result<DataValue, String>;

    fn mul(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            // 仅同类型运算，类型转换需使用 convert 节点
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a * b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a * b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a * b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a * b)),
            
            (a, b) => Err(format!(
                "Cannot multiply {:?} and {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

/// 除法运算符实现
impl Div for DataValue {
    type Output = Result<DataValue, String>;

    fn div(self, rhs: Self) -> Self::Output {
        // 检查除零
        let is_zero = match &rhs {
            DataValue::Int32(v) => *v == 0,
            DataValue::Int64(v) => *v == 0,
            DataValue::Float32(v) => *v == 0.0,
            DataValue::Float64(v) => *v == 0.0,
            _ => false,
        };
        
        if is_zero {
            return Err("Division by zero".to_string());
        }
        
        match (self, rhs) {
            // 仅同类型运算，类型转换需使用 convert 节点
            (DataValue::Int32(a), DataValue::Int32(b)) => Ok(DataValue::Int32(a / b)),
            (DataValue::Int64(a), DataValue::Int64(b)) => Ok(DataValue::Int64(a / b)),
            (DataValue::Float32(a), DataValue::Float32(b)) => Ok(DataValue::Float32(a / b)),
            (DataValue::Float64(a), DataValue::Float64(b)) => Ok(DataValue::Float64(a / b)),
            
            (a, b) => Err(format!(
                "Cannot divide {:?} by {:?}: incompatible types",
                a.value_type(),
                b.value_type()
            )),
        }
    }
}

impl DataValue {
    /// 辅助方法：执行加法运算
    pub fn add(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() + other.clone()
    }
    
    /// 辅助方法：执行减法运算
    pub fn sub(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() - other.clone()
    }
    
    /// 辅助方法：执行乘法运算
    pub fn mul(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() * other.clone()
    }
    
    /// 辅助方法：执行除法运算
    pub fn div(&self, other: &DataValue) -> Result<DataValue, String> {
        self.clone() / other.clone()
    }
}
