# Value System - 快速参考

## 概述

新的 Value 系统基于 Polars/Arrow，为 BI 系统提供更强大的类型支持和性能优化。

## 核心类型

### Value（运行时值）

```rust
pub enum Value {
    Null,                          // 空值
    Boolean(bool),                 // 布尔值
    Int64(i64),                    // 整数
    Float64(f64),                  // 浮点数
    String(String),                // 字符串
    Date(i32),                     // 日期
    Datetime(i64),                 // 时间戳
    Duration(i64),                 // 时间间隔
    List(Vec<Value>),              // 列表
    Struct(Vec<(String, Value)>),  // 结构体
    DataFrame(Arc<DataFrame>),     // DataFrame（零拷贝）
    Series(Arc<Series>),           // Series（单列）
}
```

### ValueType（类型声明）

```rust
pub enum ValueType {
    Null, Boolean, Int64, Float64, String,
    Date, Datetime, Duration,
    List(Box<ValueType>),
    Struct(Vec<(String, ValueType)>),
    DataFrame, Series,
    Any,  // 泛型类型
}
```

## 常用操作

### 创建值

```rust
let int_val = Value::Int64(42);
let float_val = Value::Float64(3.14);
let str_val = Value::String("Hello".to_string());
let bool_val = Value::Boolean(true);
let null_val = Value::Null;
```

### 类型转换

```rust
// 安全提取
let i = value.as_i64();        // Option<i64>
let f = value.as_f64();        // Option<f64>
let s = value.as_string();     // Option<String>
let b = value.as_bool();       // Option<bool>

// 跨类型转换
let int_val = Value::Int64(42);
let as_float = int_val.as_f64();  // Some(42.0)
```

### JSON 转换

```rust
use crate::executor::value::conversions::{from_json, to_json};

// JSON -> Value
let json = json!({"name": "Alice", "age": 30});
let value = from_json(&json);

// Value -> JSON
let value = Value::Int64(42);
let json = to_json(&value);
```

### DataFrame 操作

```rust
use crate::executor::value::conversions::json_to_dataframe;

// 从 JSON 创建 DataFrame
let json = json!([
    {"name": "Alice", "age": 30},
    {"name": "Bob", "age": 25}
]);
let df = json_to_dataframe(&json)?;
let value = Value::DataFrame(Arc::new(df));

// 提取 DataFrame
if let Some(df) = value.as_dataframe() {
    println!("Rows: {}, Cols: {}", df.height(), df.width());
}
```

## 类型系统特性

### 1. 类型兼容性检查

```rust
let value = Value::Int64(42);

// 检查是否兼容
if value.is_compatible_with(&ValueType::Float64) {
    // Int64 可以转换为 Float64
}

// 获取值的类型
let vtype = value.value_type();  // ValueType::Int64
```

### 2. 字符串类型解析

```rust
// 从字符串创建类型（向后兼容）
let vtype = ValueType::from_string("number");  // ValueType::Float64
let vtype = ValueType::from_string("int64");   // ValueType::Int64
let vtype = ValueType::from_string("string");  // ValueType::String
```

### 3. Polars 类型转换

```rust
// ValueType -> Polars DataType
let polars_dtype = vtype.to_polars_dtype();

// Polars DataType -> ValueType
let vtype = ValueType::from_polars_dtype(&dtype);
```

## 与旧系统对比

| 旧系统 (DataValue) | 新系统 (Value) | 说明 |
|-------------------|---------------|------|
| `None` | `Null` | 空值 |
| `Number(f64)` | `Float64(f64)` 或 `Int64(i64)` | 区分整数和浮点数 |
| `String(String)` | `String(String)` | 保持不变 |
| `Boolean(bool)` | `Boolean(bool)` | 保持不变 |
| `List(Vec<DataValue>)` | `List(Vec<Value>)` | 保持不变 |
| `Object(serde_json::Value)` | `Struct(Vec<(String, Value)>)` | 更结构化 |
| `DataFrame(Arc<serde_json::Value>)` | `DataFrame(Arc<DataFrame>)` | 真实的 Polars DataFrame |
| - | `Series(Arc<Series>)` | 新增：单列数据 |
| - | `Date(i32)` | 新增：日期类型 |
| - | `Datetime(i64)` | 新增：时间戳类型 |
| - | `Duration(i64)` | 新增：时间间隔类型 |

## 优势

### 1. 类型安全
- ✅ 编译时类型检查
- ✅ 明确的整数和浮点数区分
- ✅ 更好的错误处理

### 2. 性能优化
- ✅ DataFrame 零拷贝传递（Arc）
- ✅ 原生 Polars 类型，无需序列化
- ✅ 更高效的内存使用

### 3. BI 系统集成
- ✅ 直接支持 Polars DataFrame
- ✅ 支持日期、时间戳等 BI 常用类型
- ✅ 与 Arrow 生态系统兼容

### 4. 可扩展性
- ✅ 易于添加新的数据类型
- ✅ 支持复杂的嵌套结构
- ✅ 与 Polars 功能无缝集成

## 使用示例

### 示例 1: 数学节点

```rust
// 加法节点
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    Value::Float64(a + b)
}));
```

### 示例 2: 多输出节点

```rust
// DivMod 节点
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let dividend = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let divisor = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(1.0);
    
    if divisor == 0.0 {
        return Value::Null;
    }
    
    // 根据请求的输出 Pin 返回不同的值
    let output_name = node.outputs.iter()
        .find(|p| p.id == *pin_id)
        .map(|p| p.name.as_str())
        .unwrap_or("");
    
    match output_name {
        "Quotient" => Value::Float64((dividend / divisor).floor()),
        "Remainder" => Value::Float64(dividend % divisor),
        _ => Value::Null
    }
}));
```

### 示例 3: DataFrame 节点

```rust
// 创建 DataFrame 节点
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let json_data = ctx.get_pin_value(&node.inputs[0].id);
    
    match json_to_dataframe(&to_json(&json_data)) {
        Ok(df) => Value::DataFrame(Arc::new(df)),
        Err(_) => Value::Null
    }
}));
```

## 文件位置

- **类型定义**: `src/executor/value/types.rs`
- **转换函数**: `src/executor/value/conversions.rs`
- **模块导出**: `src/executor/value/mod.rs`
- **详细文档**: `VALUE_SYSTEM.md`
- **迁移计划**: `VALUE_MIGRATION_PLAN.md`

## 测试

```bash
cd src-tauri
cargo test value::
```

## 下一步

查看 [VALUE_MIGRATION_PLAN.md](./VALUE_MIGRATION_PLAN.md) 了解完整的迁移计划。

当前状态：**阶段 1 完成** ✅

下一步：**阶段 2 - Pin 系统迁移**
