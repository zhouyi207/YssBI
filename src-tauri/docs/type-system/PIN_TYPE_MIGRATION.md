# Pin Type Migration - 快速修改指南

## 问题

所有节点定义中使用字符串类型创建 Pin：
```rust
GenericInDataPin::new(uuid::Uuid::nil(), "Input", "number")  // ❌ 错误
GenericOutDataPin::new(uuid::Uuid::nil(), "Output", "string")  // ❌ 错误
```

现在需要改为 `ValueType`：
```rust
use crate::executor::value::ValueType;

GenericInDataPin::new(uuid::Uuid::nil(), "Input", ValueType::Float64)  // ✅ 正确
GenericOutDataPin::new(uuid::Uuid::nil(), "Output", ValueType::String)  // ✅ 正确
```

## 类型映射表

| 旧字符串类型 | 新 ValueType | 说明 |
|------------|-------------|------|
| `"number"` | `ValueType::Float64` | 浮点数 |
| `"int"` | `ValueType::Int64` | 整数 |
| `"string"` | `ValueType::String` | 字符串 |
| `"bool"` | `ValueType::Boolean` | 布尔值 |
| `"array"` | `ValueType::List(Box::new(ValueType::Any))` | 列表 |
| `"object"` | `ValueType::Struct(vec![])` | 对象/结构体 |
| `"any"` | `ValueType::Any` | 任意类型 |
| `"dataframe"` | `ValueType::DataFrame` | DataFrame |

## 需要修改的文件列表

### 核心节点
- [ ] `src/executor/node/catalog/math/operators.rs`
- [ ] `src/executor/node/catalog/math/multi_output.rs`
- [ ] `src/executor/node/catalog/data.rs`
- [ ] `src/executor/node/catalog/variable.rs`
- [ ] `src/executor/node/catalog/debug.rs`
- [ ] `src/executor/node/catalog/control.rs`
- [ ] `src/executor/node/catalog/function.rs`
- [ ] `src/executor/node/catalog/internal.rs`
- [ ] `src/executor/node/catalog/visualization.rs`

### 多输出节点
- [ ] `src/executor/node/catalog/string_multi_output.rs`
- [ ] `src/executor/node/catalog/data_multi_output.rs`

### 测试文件
- [ ] `tests/multi_output_node_test.rs`
- [ ] `tests/control_flow_unit_tests.rs`
- [ ] `tests/basic_node_test.rs`

### ExecutionContext
- [ ] `src/executor/context.rs` - 需要修改 Pin 创建逻辑

## 修改步骤

### 1. 添加导入
在每个文件顶部添加：
```rust
use crate::executor::value::ValueType;
```

### 2. 批量替换

使用以下正则表达式替换：

**查找：** `"number"`  
**替换为：** `ValueType::Float64`

**查找：** `"string"`  
**替换为：** `ValueType::String`

**查找：** `"bool"`  
**替换为：** `ValueType::Boolean`

**查找：** `"array"`  
**替换为：** `ValueType::List(Box::new(ValueType::Any))`

**查找：** `"object"`  
**替换为：** `ValueType::Struct(vec![])`

**查找：** `"any"`  
**替换为：** `ValueType::Any`

### 3. 特殊情况处理

#### ExecutionContext.rs
在 `create_node_from_data` 方法中：
```rust
// 旧代码
let pin = GenericInDataPin::new(runtime_id, &pin_data.name, &pin_data.pin_type);

// 新代码
let pin = GenericInDataPin::new(
    runtime_id, 
    &pin_data.name, 
    ValueType::from_string(&pin_data.pin_type)
);
```

## 验证

修改完成后运行：
```bash
cd src-tauri
cargo check
cargo test
```

## 注意事项

1. **不要修改字符串字面量中的类型名称**（如日志消息）
2. **只修改 Pin 构造函数的参数**
3. **确保添加了 `use crate::executor::value::ValueType;`**
4. **`ValueType::List` 和 `ValueType::Struct` 需要额外的参数**

## 示例

### 修改前
```rust
let node = GenericNode::new_prototype("add", "Add");
node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "A", "number"));
node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "B", "number"));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", "number"));
```

### 修改后
```rust
use crate::executor::value::ValueType;

let node = GenericNode::new_prototype("add", "Add");
node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "A", ValueType::Float64));
node.add_input(GenericInDataPin::new(uuid::Uuid::nil(), "B", ValueType::Float64));
node.add_output(GenericOutDataPin::new(uuid::Uuid::nil(), "Result", ValueType::Float64));
```
