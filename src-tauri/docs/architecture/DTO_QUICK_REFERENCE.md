# DTO 快速参考指南

## 什么是 DTO？

DTO (Data Transfer Object) 是用于在不同层之间传输数据的对象，只包含数据，不包含业务逻辑。

## 项目中的 DTO

### 位置
```
src/project/dto.rs
```

### 导入方式
```rust
// 方式 1：直接从 project 导入
use crate::project::{NodeDto, PinDto, GraphDto, VariableDto, PinDefDto};

// 方式 2：从 executor 重新导出（为了兼容性）
use crate::executor::{NodeDto, PinDto, GraphDto, VariableDto, PinDefDto};
```

## DTO 列表

### NodeDto
节点数据传输对象，用于执行时的图表示。

```rust
pub struct NodeDto {
    pub id: String,
    pub node_type: String,
    pub title: String,
    pub inputs: Vec<PinDto>,
    pub outputs: Vec<PinDto>,
    pub variable_id: Option<String>,
    pub sub_graph_id: Option<String>,
}
```

**用途**：
- 执行器输入
- 前端与后端的节点数据交换
- 不包含 UI 信息（如位置）

### PinDto
Pin 数据传输对象。

```rust
pub struct PinDto {
    pub id: String,
    pub name: String,
    pub pin_type: String,
    pub links: Vec<String>,
    pub default_value: Option<Value>,
    pub is_array: bool,
}
```

**用途**：
- 节点的输入输出 Pin 数据
- 连接信息传输

### GraphDto
图数据传输对象，用于执行时的完整图表示。

```rust
pub struct GraphDto {
    pub version: String,
    pub nodes: Vec<NodeDto>,
    pub variables: Option<HashMap<String, VariableDto>>,
}
```

**用途**：
- 执行器的输入格式
- 简化的图表示（不包含子图、数据帧等）

### VariableDto
变量数据传输对象。

```rust
pub struct VariableDto {
    pub name: String,
    pub var_type: String,
    pub value: Value,
}
```

**用途**：
- 变量值传输
- 执行时的变量表示

### PinDefDto
Pin 定义数据传输对象。

```rust
pub struct PinDefDto {
    pub name: String,
    pub pin_type: String,
    pub default_value: Option<Value>,
    pub is_array: bool,
}
```

**用途**：
- 函数/宏的输入输出参数定义
- 节点原型定义

## DTO vs 其他结构

### NodeDto vs SerializedNode

| 特性 | NodeDto | SerializedNode |
|------|---------|----------------|
| 位置 | `project/dto.rs` | `project/mod.rs` |
| 用途 | 执行时图表示 | 项目文件持久化 |
| 包含位置 | ❌ | ✅ position |
| 包含 UI 信息 | ❌ | ✅ isInternal, variableName |
| 使用场景 | 执行器输入 | 保存/加载项目 |

### NodeDto vs GenericNode

| 特性 | NodeDto | GenericNode |
|------|---------|-------------|
| 位置 | `project/dto.rs` | `executor/node/implementation.rs` |
| 类型 | 贫血模型 | 充血模型 |
| 包含逻辑 | ❌ | ✅ 处理器、执行逻辑 |
| 可序列化 | ✅ | ✅ (部分) |
| 运行时状态 | ❌ | ✅ Pin 实例、状态 |
| 使用场景 | 数据传输 | 图执行 |

## 数据流

```
前端 JSON
    ↓ 反序列化
SerializedNode (项目文件格式)
    ↓ 转换
NodeDto (执行格式)
    ↓ 创建运行时对象
GenericNode (运行时)
    ↓ 执行
结果
    ↓ 序列化
前端 JSON
```

## 常见操作

### 从 JSON 创建 NodeDto
```rust
let node_dto: NodeDto = serde_json::from_str(json_str)?;
```

### NodeDto 转 JSON
```rust
let json = serde_json::to_string(&node_dto)?;
```

### SerializedNode 转 NodeDto
```rust
// 需要手动转换，因为结构不同
let node_dto = NodeDto {
    id: serialized_node.id,
    node_type: serialized_node.node_type,
    title: serialized_node.title,
    inputs: serialized_node.inputs.into_iter()
        .map(|p| PinDto { /* ... */ })
        .collect(),
    // ...
};
```

### NodeDto 创建 GenericNode
```rust
// 通过节点注册表
let prototype = registry.get_node(&node_dto.node_type)?;
let runtime_node = prototype.clone_with_id(node_id);
```

## 最佳实践

1. **使用 DTO 进行数据传输**
   - 前端 ↔ 后端：使用 DTO
   - 模块间传递数据：使用 DTO
   - 执行逻辑：使用运行时对象（GenericNode）

2. **不要在 DTO 中添加业务逻辑**
   - DTO 只包含数据和序列化逻辑
   - 业务逻辑放在服务层或运行时对象中

3. **保持 DTO 简单**
   - 避免复杂的嵌套
   - 使用 Option 处理可选字段
   - 使用 serde 属性控制序列化

4. **版本兼容性**
   - 添加新字段时使用 `#[serde(default)]`
   - 重命名字段时使用 `#[serde(rename = "oldName")]`
   - 考虑向后兼容性

## 相关文档

- [DTO 重构总结](./DTO_REFACTOR_SUMMARY.md)
- [执行器设计](./EXECUTOR_DESIGN.md)
- [类型系统快速指南](../type-system/TYPE_SYSTEM_QUICK_GUIDE.md)
