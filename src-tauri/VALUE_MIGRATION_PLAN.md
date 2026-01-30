# Value System Migration Plan

## 目标

将执行器从基于 `serde_json::Value` 和 `DataValue` 的类型系统迁移到基于 Polars/Arrow 的 `Value` 和 `ValueType` 系统。

## 当前状态

✅ **已完成：**
1. 创建 `src/executor/value/` 模块
2. 定义 `Value` 枚举（基于 Polars 类型）
3. 定义 `ValueType` 枚举（用于 Pin 类型声明）
4. 实现 JSON ↔ Value 转换函数
5. 实现 Polars AnyValue ↔ Value 转换函数
6. 添加类型检查和兼容性方法
7. 代码编译通过

## 迁移阶段

### 阶段 1: 基础设施（当前阶段）✅

**目标：** 创建新的类型系统，不破坏现有代码

**任务：**
- [x] 创建 `value` 模块
- [x] 定义 `Value` 和 `ValueType`
- [x] 实现转换函数
- [x] 编写文档

**影响：** 无，新代码与旧代码隔离

---

### 阶段 2: Pin 系统迁移

**目标：** 更新 Pin 使用新的类型系统

**任务：**

#### 2.1 更新 Pin 实现
- [ ] 修改 `GenericInDataPin::data_type` 从 `String` 改为 `ValueType`
- [ ] 修改 `GenericOutDataPin::data_type` 从 `String` 改为 `ValueType`
- [ ] 修改 `GenericInDataPin::value` 从 `RwLock<DataValue>` 改为 `RwLock<Value>`
- [ ] 修改 `GenericOutDataPin::value` 从 `RwLock<DataValue>` 改为 `RwLock<Value>`

**文件：**
- `src/executor/pin/implementation.rs`
- `src/executor/pin/traits.rs`

#### 2.2 更新 Pin 构造函数
```rust
// 旧代码
GenericInDataPin::new(node_id, "Input", "number")

// 新代码（选项 1：字符串，自动转换）
GenericInDataPin::new(node_id, "Input", "number")  // 内部调用 ValueType::from_string()

// 新代码（选项 2：直接使用 ValueType）
GenericInDataPin::new(node_id, "Input", ValueType::Float64)
```

**建议：** 保持字符串接口，内部转换为 `ValueType`，保证向后兼容

#### 2.3 更新 DataPin trait
```rust
// 旧代码
fn value(&self) -> DataValue;
fn set_value(&self, value: DataValue) -> NodeResult<()>;
fn data_type(&self) -> &str;

// 新代码
fn value(&self) -> Value;
fn set_value(&self, value: Value) -> NodeResult<()>;
fn data_type(&self) -> &ValueType;
```

**影响：** 所有使用 Pin 的代码需要更新

---

### 阶段 3: ExecutionContext 迁移

**目标：** 更新执行上下文使用新的 Value 系统

**任务：**

#### 3.1 更新 data_cache
```rust
// 旧代码
data_cache: HashMap<PinId, serde_json::Value>

// 新代码
data_cache: HashMap<PinId, Value>
```

#### 3.2 更新 ExecutionContextTrait
```rust
// 旧代码
fn get_pin_value(&mut self, pin_id_str: &str) -> serde_json::Value;
fn set_variable(&mut self, var_id: &str, value: serde_json::Value) -> bool;

// 新代码
fn get_pin_value(&mut self, pin_id_str: &str) -> Value;
fn set_variable(&mut self, var_id: &str, value: Value) -> bool;
```

#### 3.3 更新 variables 存储
```rust
// 旧代码
variables: HashMap<String, serde_json::Value>

// 新代码
variables: HashMap<String, Value>
```

**文件：**
- `src/executor/context.rs`
- `src/executor/processors.rs`

**影响：** 所有节点处理器需要更新

---

### 阶段 4: 节点处理器迁移

**目标：** 更新所有节点使用新的 Value 系统

**策略：** 分批迁移，优先迁移核心节点

#### 4.1 核心节点（优先级：高）
- [ ] Math 节点 (`src/executor/node/catalog/math/operators.rs`)
  - Add, Subtract, Multiply, Divide
  - 使用 `Value::Float64` 和 `Value::Int64`
- [ ] Constant 节点 (`src/executor/node/catalog/data.rs`)
  - 根据输入类型创建对应的 Value
- [ ] Variable 节点 (`src/executor/node/catalog/variable.rs`)
  - GetVariable, SetVariable

#### 4.2 控制流节点（优先级：中）
- [ ] IfElse (`src/executor/node/catalog/control.rs`)
- [ ] Sequence, Sequence5
- [ ] WhileLoop, ForLoop

#### 4.3 多输出节点（优先级：中）
- [ ] Math 多输出 (`src/executor/node/catalog/math/multi_output.rs`)
  - DivMod, MinMax, SinCos, etc.
- [ ] String 多输出 (`src/executor/node/catalog/string_multi_output.rs`)
- [ ] Data 多输出 (`src/executor/node/catalog/data_multi_output.rs`)

#### 4.4 其他节点（优先级：低）
- [ ] Debug 节点
- [ ] Function 节点
- [ ] Visualization 节点

**迁移模板：**

```rust
// 旧代码
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    json!(a + b)
}));

// 新代码
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id).as_f64().unwrap_or(0.0);
    let b = ctx.get_pin_value(&node.inputs[1].id).as_f64().unwrap_or(0.0);
    Value::Float64(a + b)
}));
```

---

### 阶段 5: 前端集成

**目标：** 确保前端可以正确处理新的类型系统

**任务：**

#### 5.1 更新 Tauri Commands
- [ ] 确保序列化/反序列化正确
- [ ] 添加 Value ↔ JSON 转换层

#### 5.2 更新 Schema
- [ ] 更新 Pin 类型定义
- [ ] 添加新的类型选项（Date, Datetime, Duration, etc.）

**文件：**
- `src/schema/pin_types.rs`
- Tauri command handlers

---

### 阶段 6: 测试和验证

**目标：** 确保迁移后系统正常工作

**任务：**

#### 6.1 单元测试
- [ ] Value 类型转换测试
- [ ] Pin 类型检查测试
- [ ] ExecutionContext 测试

#### 6.2 集成测试
- [ ] 更新现有测试使用新类型
- [ ] 添加 DataFrame 节点测试
- [ ] 性能测试

#### 6.3 回归测试
- [ ] 运行所有现有测试
- [ ] 验证多输出节点
- [ ] 验证控制流节点

**文件：**
- `tests/` 目录下所有测试文件

---

### 阶段 7: 清理和优化

**目标：** 移除旧代码，优化性能

**任务：**

#### 7.1 移除旧类型
- [ ] 移除 `DataValue` 枚举（如果不再使用）
- [ ] 移除不必要的转换代码
- [ ] 更新文档

#### 7.2 性能优化
- [ ] 优化 DataFrame 传递
- [ ] 减少不必要的克隆
- [ ] 优化缓存策略

#### 7.3 文档更新
- [ ] 更新 EXECUTOR_DESIGN.md
- [ ] 更新 API 文档
- [ ] 添加迁移指南

---

## 风险和缓解

### 风险 1: 破坏现有功能
**缓解：**
- 分阶段迁移，每个阶段都运行测试
- 保持向后兼容的转换层
- 使用 feature flags 控制新旧代码

### 风险 2: 性能下降
**缓解：**
- 在每个阶段进行性能测试
- 使用 Arc 避免不必要的克隆
- 优化热路径

### 风险 3: 前端兼容性问题
**缓解：**
- 保持 JSON 序列化格式不变
- 添加转换层处理新旧格式
- 渐进式更新前端

---

## 时间估算

| 阶段 | 预计时间 | 状态 |
|------|---------|------|
| 阶段 1: 基础设施 | 2 小时 | ✅ 完成 |
| 阶段 2: Pin 系统 | 3 小时 | ⏳ 待开始 |
| 阶段 3: ExecutionContext | 2 小时 | ⏳ 待开始 |
| 阶段 4: 节点处理器 | 6 小时 | ⏳ 待开始 |
| 阶段 5: 前端集成 | 3 小时 | ⏳ 待开始 |
| 阶段 6: 测试验证 | 4 小时 | ⏳ 待开始 |
| 阶段 7: 清理优化 | 2 小时 | ⏳ 待开始 |
| **总计** | **22 小时** | |

---

## 下一步行动

**立即执行（阶段 2）：**

1. 更新 `GenericInDataPin` 和 `GenericOutDataPin`
   - 修改 `data_type` 字段类型
   - 修改 `value` 字段类型
   - 更新构造函数支持字符串到 ValueType 的转换

2. 更新 `DataPin` trait
   - 修改方法签名
   - 保持向后兼容

3. 运行测试确保没有破坏现有功能

**命令：**
```bash
cd src-tauri
cargo test
```

---

## 参考文档

- [VALUE_SYSTEM.md](./VALUE_SYSTEM.md) - 新类型系统文档
- [EXECUTOR_DESIGN.md](./EXECUTOR_DESIGN.md) - 执行器设计文档
- [Polars 文档](https://pola-rs.github.io/polars-book/)
