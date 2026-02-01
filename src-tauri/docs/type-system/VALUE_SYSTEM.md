# Value System - Polars/Arrow 类型集成

## 概述

新的 Value 系统基于 Polars/Arrow 类型系统，为 BI 系统提供更强大的数据处理能力。

## 核心组件

### 1. Value 枚举 (`src/executor/value/types.rs`)

```rust
pub enum Value {
    Null,
    Boolean(bool),
    Int64(i64),
    Float64(f64),
    String(String),
    Date(i32),
    Datetime(i64),
    Duration(i64),
    List(Vec<Value>),
    Struct(Vec<(String, Value)>),
    DataFrame(Arc<DataFrame>),  // 零拷贝传递
    Series(Arc<Series>),
}
```

### 2. ValueType 枚举

用于 Pin 的 `data_type` 字段，描述期望的数据类型：

```rust
pub enum ValueType {
    Null,
    Boolean,
    Int64,
    Float64,
    String,
    Date,
    Datetime,
    Duration,
    List(Box<ValueType>),
    Struct(Vec<(String, ValueType)>),
    DataFrame,
    Series,
    Any,  // 泛型类型
}
```

## 主要特性

### 1. 类型转换

#### 从 JSON 转换
```rust
use crate::executor::value::conversions::from_json;

let json = json!({"name": "Alice", "age": 30});
let value = from_json(&json);
```

#### 转换为 JSON
```rust
use crate::executor::value::conversions::to_json;

let value = Value::Int64(42);
let json = to_json(&value);
```

#### 从 Polars AnyValue 转换
```rust
use crate::executor::value::conversions::from_polars;

let any_value = AnyValue::Int64(42);
let value = from_polars(any_value);
```

### 2. 类型检查

```rust
let value = Value::Int64(42);

// 检查类型兼容性
if value.is_compatible_with(&ValueType::Float64) {
    // Int64 可以转换为 Float64
}

// 获取值类型
let vtype = value.value_type();  // ValueType::Int64
```

### 3. 值提取

```rust
let value = Value::Int64(42);

// 安全提取
if let Some(i) = value.as_i64() {
    println!("Integer: {}", i);
}

// 类型转换
if let Some(f) = value.as_f64() {
    println!("As float: {}", f);  // 42.0
}
```

### 4. DataFrame 支持

```rust
use crate::executor::value::conversions::json_to_dataframe;

let json = json!([
    {"name": "Alice", "age": 30},
    {"name": "Bob", "age": 25}
]);

let df = json_to_dataframe(&json)?;
let value = Value::DataFrame(Arc::new(df));
```

## 迁移指南

### 当前系统 (DataValue)

```rust
// 旧代码
pub enum DataValue {
    None,
    Number(f64),
    String(String),
    Boolean(bool),
    List(Vec<DataValue>),
    Object(serde_json::Value),
    DataFrame(Arc<serde_json::Value>),
}
```

### 新系统 (Value)

```rust
// 新代码
pub enum Value {
    Null,                          // 替代 None
    Int64(i64),                    // 新增：整数类型
    Float64(f64),                  // 替代 Number
    String(String),                // 保持不变
    Boolean(bool),                 // 保持不变
    List(Vec<Value>),              // 保持不变
    Struct(Vec<(String, Value)>),  // 替代 Object
    DataFrame(Arc<DataFrame>),     // 使用真实的 Polars DataFrame
    Series(Arc<Series>),           // 新增：单列数据
    Date(i32),                     // 新增：日期类型
    Datetime(i64),                 // 新增：时间戳类型
    Duration(i64),                 // 新增：时间间隔类型
}
```

### 迁移步骤

#### 1. 更新 Pin 定义

**旧代码：**
```rust
pub struct GenericInDataPin {
    data_type: String,  // "number", "string", etc.
    value: RwLock<DataValue>,
}
```

**新代码：**
```rust
pub struct GenericInDataPin {
    data_type: ValueType,  // ValueType::Float64, ValueType::String, etc.
    value: RwLock<Value>,
}
```

#### 2. 更新节点处理器

**旧代码：**
```rust
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    json!(a + b)
}));
```

**新代码（过渡期）：**
```rust
use crate::executor::value::conversions::{from_json, to_json};

node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let json_a = ctx.get_pin_value(&node.inputs[0].id);
    let json_b = ctx.get_pin_value(&node.inputs[1].id);
    
    let a = from_json(&json_a).as_f64().unwrap_or(0.0);
    let b = from_json(&json_b).as_f64().unwrap_or(0.0);
    
    to_json(&Value::Float64(a + b))
}));
```

**新代码（完全迁移后）：**
```rust
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    Value::Float64(a + b)
}));
```

#### 3. 更新 ExecutionContext

**需要修改的方法：**
- `get_pin_value()` - 返回 `Value` 而不是 `serde_json::Value`
- `set_variable()` - 接受 `Value` 而不是 `serde_json::Value`
- `data_cache` - 存储 `Value` 而不是 `serde_json::Value`

## 优势

### 1. 类型安全
- 编译时类型检查
- 明确的整数和浮点数区分
- 更好的错误处理

### 2. 性能优化
- DataFrame 零拷贝传递（使用 Arc）
- 原生 Polars 类型，无需序列化/反序列化
- 更高效的内存使用

### 3. BI 系统集成
- 直接支持 Polars DataFrame
- 支持日期、时间戳等 BI 常用类型
- 与 Arrow 生态系统兼容

### 4. 可扩展性
- 易于添加新的数据类型
- 支持复杂的嵌套结构
- 与 Polars 功能无缝集成

## 向后兼容

为了保持向后兼容，提供了转换函数：

```rust
// JSON -> Value
let value = from_json(&json_value);

// Value -> JSON
let json = to_json(&value);
```

这允许渐进式迁移，新旧代码可以共存。

## 最佳实践

### 1. 使用类型推断
```rust
// 好
let value = Value::Int64(42);

// 避免
let value = from_json(&json!(42));
```

### 2. DataFrame 使用 Arc
```rust
// 好 - 零拷贝
let df_value = Value::DataFrame(Arc::new(df));

// 避免 - 会克隆整个 DataFrame
let df_clone = df.clone();
```

### 3. 类型检查
```rust
// 好 - 使用类型检查
if value.is_compatible_with(&ValueType::Float64) {
    let f = value.as_f64().unwrap();
}

// 避免 - 直接 unwrap
let f = value.as_f64().unwrap();  // 可能 panic
```

### 4. 错误处理
```rust
// 好
match value.as_f64() {
    Some(f) => println!("Float: {}", f),
    None => println!("Not a float"),
}

// 避免
let f = value.as_f64().unwrap();
```

## 测试

运行测试：
```bash
cd src-tauri
cargo test value::
```

## 下一步

1. ✅ 创建 Value 模块和类型定义
2. ⏳ 更新 Pin 实现使用 ValueType
3. ⏳ 更新 ExecutionContext 使用 Value
4. ⏳ 迁移现有节点处理器
5. ⏳ 添加 DataFrame 节点
6. ⏳ 性能测试和优化

## 参考

- [Polars 文档](https://pola-rs.github.io/polars-book/)
- [Arrow 类型系统](https://arrow.apache.org/docs/format/Columnar.html)
- `src/executor/value/types.rs` - 类型定义
- `src/executor/value/conversions.rs` - 转换函数
