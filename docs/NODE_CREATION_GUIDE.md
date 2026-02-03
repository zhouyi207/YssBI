# Node 创建指南

本文档详细说明了在 YSSBI 项目中创建一个新节点所涉及的结构体、流程和最佳实践。

## 目录

1. [核心概念](#核心概念)
2. [涉及的主要结构体](#涉及的主要结构体)
3. [Node 创建流程](#node-创建流程)
4. [完整示例](#完整示例)
5. [高级特性](#高级特性)
6. [最佳实践](#最佳实践)

---

## 核心概念

### 什么是 Node？

Node（节点）是可视化编程系统的基本单元，代表一个可执行的操作或功能。每个节点通过 Pin（引脚）与其他节点连接，形成数据流或控制流图。

### Node 的组成部分

1. **基本信息**：ID、类型、标题、分类、UI 样式
2. **Pin（引脚）**：
   - **Data Pin**：传递数据的引脚（输入/输出）
   - **Exec Pin**：控制执行流的引脚（输入/输出）
3. **处理器**：
   - **Flow Processor**：处理控制流逻辑
   - **Data Processor**：处理数据计算逻辑
4. **元数据**：分类、描述、变量关联等

### 重要架构原则

**Pin 与连接的分离**：

- **Pin 仅表示端口**：Pin 只负责存储类型、状态和值，不存储任何连接关系
- **Graph 管理连接**：所有连接（Data / Exec）都由 Graph/ConnectionManager 统一管理
- **单一真实来源**：ConnectionManager 是连接关系的唯一真实来源（Single Source of Truth）
- **禁止字段**：Pin 中不允许出现 `upstream`、`downstream`、`edges`、`links` 等连接相关字段

这种设计确保了：
- 连接管理的集中化和一致性
- 更容易进行循环检测和类型推断
- 避免 Pin 之间的状态不一致问题

---

## 涉及的主要结构体

### 1. GenericNode

**位置**：`src-tauri/src/executor/node/implementation.rs`

**作用**：通用节点容器，是所有节点的统一实现

**核心字段**：

```rust
pub struct GenericNode {
    id: NodeId,                                    // 节点唯一标识
    title: RwLock<String>,                         // 节点标题
    node_type: String,                             // 节点类型（如 "add", "if_else"）
    state: RwLock<NodeState>,                      // 节点状态
    
    // 元数据
    category: Vec<String>,                         // 分类路径（如 ["Math", "Operators"]）
    ui_style: String,                              // UI 样式标识
    description: Option<String>,                   // 节点描述
    
    // Pin 存储（使用 DashMap 支持并发访问）
    in_data_pins: DashMap<PinId, Arc<GenericInDataPin>>,
    out_data_pins: DashMap<PinId, Arc<GenericOutDataPin>>,
    in_exec_pins: DashMap<PinId, Arc<GenericInExecPin>>,
    out_exec_pins: DashMap<PinId, Arc<GenericOutExecPin>>,
    
    // Pin 顺序追踪
    input_order: RwLock<Vec<PinId>>,
    output_order: RwLock<Vec<PinId>>,
    
    // 处理器
    flow_processor: Mutex<Option<FlowProcessor>>,
    data_processor: Mutex<Option<DataProcessor>>,
    
    // 动态 Pin 支持（高级特性）
    dynamic_capability: RwLock<Option<NodeDynamicCapability>>,
    dynamic_pins: RwLock<HashMap<PinId, DynamicPinInfo>>,
}
```

**关键方法**：

- `new_prototype()` - 创建节点原型（用于注册）
- `new()` - 创建节点实例（用于运行时）
- `add_in_data_pin()` - 添加输入数据 Pin
- `add_out_data_pin()` - 添加输出数据 Pin
- `add_in_exec_pin()` - 添加输入执行 Pin
- `add_out_exec_pin()` - 添加输出执行 Pin
- `set_flow_processor()` - 设置控制流处理器
- `set_data_processor()` - 设置数据处理器
- `set_metadata()` - 设置元数据

### 2. Pin 相关结构体

**位置**：`src-tauri/src/executor/pin/implementation.rs`

#### GenericInDataPin - 输入数据 Pin

```rust
pub struct GenericInDataPin {
    id: PinId,                                     // Pin 唯一标识
    node_id: NodeId,                               // 所属节点 ID
    name: String,                                  // Pin 名称
    type_desc: PinTypeDesc,                        // 类型描述
    state: RwLock<DataPinState>,                   // Pin 状态
    value: RwLock<DataValue>,                      // 当前值
    // 注意：不再存储 upstream 连接信息
    // 所有连接由 Graph/ConnectionManager 统一管理
}
```

#### GenericOutDataPin - 输出数据 Pin

```rust
pub struct GenericOutDataPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    type_desc: PinTypeDesc,
    state: RwLock<DataPinState>,
    value: RwLock<DataValue>,
    // 注意：不再存储 downstream 连接信息
    // 所有连接由 Graph/ConnectionManager 统一管理
}
```

#### GenericInExecPin - 输入执行 Pin

```rust
pub struct GenericInExecPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    state: RwLock<ExecPinState>,
    // 注意：不再存储 upstream 连接信息
    // 所有连接由 Graph/ConnectionManager 统一管理
}
```

#### GenericOutExecPin - 输出执行 Pin

```rust
pub struct GenericOutExecPin {
    id: PinId,
    node_id: NodeId,
    name: String,
    state: RwLock<ExecPinState>,
    // 注意：不再存储 downstream 连接信息
    // 所有连接由 Graph/ConnectionManager 统一管理
}
```

### 3. PinTypeDesc - Pin 类型描述

**位置**：`src-tauri/src/executor/value/pin_type.rs`

**作用**：描述 Pin 的数据类型，支持具体类型、类型变量和类型约束

```rust
pub struct PinTypeDesc {
    pub data_type: DataType,                       // 数据类型
    pub constraints: Vec<TypeConstraint>,          // 类型约束
    pub is_optional: bool,                         // 是否可选
    pub is_array: bool,                            // 是否数组
}
```

**创建方法**：

```rust
// 具体类型
PinTypeDesc::concrete(ValueType::Float64)

// 类型变量（用于泛型）
PinTypeDesc::type_var(type_var_id)

// 带约束的类型变量
PinTypeDesc::type_var_with_constraints(type_var_id, vec![TypeConstraint::Numeric])

// 未知类型
PinTypeDesc::unknown()
```

### 4. NodeRegistry - 节点注册中心

**位置**：`src-tauri/src/executor/node/registry.rs`

**作用**：管理所有节点原型的注册和获取

```rust
pub struct NodeRegistry {
    prototypes: RwLock<HashMap<String, Arc<GenericNode>>>,
}
```

**关键方法**：

- `register()` - 注册节点原型
- `get_prototype()` - 获取节点原型
- `get_all_prototypes()` - 获取所有节点原型

### 5. ConnectionManager - 连接管理器

**位置**：`src-tauri/src/executor/connection.rs`

**作用**：管理图中所有节点间的连接，是连接关系的唯一真实来源

```rust
pub struct ConnectionManager {
    /// 所有连接（from_pin -> to_pin）
    connections: Mutex<HashMap<PinId, Vec<PinId>>>,
    
    /// Pin 到节点的映射
    pin_to_node: Mutex<HashMap<PinId, NodeId>>,
    
    /// 节点到 Pin 的映射
    node_to_pins: Mutex<HashMap<NodeId, Vec<PinId>>>,
    
    /// 类型推断上下文
    type_inference: Mutex<TypeInferenceContext>,
}
```

**关键方法**：

- `connect()` - 连接两个 Pin（带类型检查）
- `connect_by_id()` - 直接通过 PinId 建立连接
- `disconnect()` - 断开连接
- `get_downstream()` - 获取 Pin 的所有下游连接
- `get_upstream()` - 获取 Pin 的上游连接
- `get_upstream_nodes()` - 获取节点的所有直接上游节点
- `get_downstream_nodes()` - 获取节点的所有直接下游节点

**连接规则**：

- **Data In Pin**：最多 1 条输入边（ConnectionManager 自动保证）
- **Data Out Pin**：可以有多条输出边
- **Exec In Pin**：最多 1 条输入边
- **Exec Out Pin**：可以有多条输出边

### 6. DTO 结构体

**位置**：`src-tauri/src/project/dto.rs`

#### NodeDto - 节点数据传输对象

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

#### PinDto - Pin 数据传输对象

```rust
pub struct PinDto {
    pub id: String,
    pub name: String,
    pub pin_type: String,
    pub default_value: Option<Value>,
    pub user_value: Option<Value>,
    pub is_array: bool,
    pub show_widget: bool,
    pub widget_type: Option<String>,
}
```

---

## Node 创建流程

### 流程图

```
1. 创建节点原型 (GenericNode::new_prototype)
   ↓
2. 添加 Pin (add_in_data_pin, add_out_data_pin, etc.)
   ↓
3. 设置处理器 (set_data_processor / set_flow_processor)
   ↓
4. 设置元数据 (set_metadata)
   ↓
5. 注册到注册中心 (registry.register)
   ↓
6. 运行时：从原型克隆实例
```

### 详细步骤

#### 步骤 1：创建节点原型

```rust
let node = GenericNode::new_prototype("add", "Add (+)");
```

- 使用 `new_prototype()` 创建原型（ID 为 nil UUID）
- 第一个参数：节点类型（唯一标识符）
- 第二个参数：节点标题（显示名称）

#### 步骤 2：添加 Pin

**添加输入数据 Pin**：

```rust
node.add_in_data_pin(GenericInDataPin::new(
    uuid::Uuid::nil(),                             // 原型使用 nil UUID
    "A",                                           // Pin 名称
    PinTypeDesc::concrete(ValueType::Float64)      // Pin 类型
));
```

**添加输出数据 Pin**：

```rust
node.add_out_data_pin(GenericOutDataPin::new(
    uuid::Uuid::nil(),
    "Result",
    PinTypeDesc::concrete(ValueType::Float64)
));
```

**添加执行 Pin**：

```rust
// 输入执行 Pin
node.add_in_exec_pin(GenericInExecPin::new(
    uuid::Uuid::nil(),
    "In"
));

// 输出执行 Pin
node.add_out_exec_pin(GenericOutExecPin::new(
    uuid::Uuid::nil(),
    "Then"
));
```

#### 步骤 3：设置处理器

**数据处理器**（用于纯数据节点）：

```rust
node.set_data_processor(Box::new(|ctx, node, _pin_id| {
    // 获取输入值
    let a = ctx.get_pin_value(&node.inputs[0].id);
    let b = ctx.get_pin_value(&node.inputs[1].id);
    
    // 执行计算
    let va = a.as_f64().unwrap_or(0.0);
    let vb = b.as_f64().unwrap_or(0.0);
    
    // 返回结果
    Value::from(va + vb)
}));
```

**控制流处理器**（用于有执行 Pin 的节点）：

```rust
node.set_flow_processor(Box::new(|ctx, node| {
    // 获取输入值
    let condition = ctx.get_pin_value(&node.inputs[0].id);
    
    // 根据条件选择输出执行 Pin
    if condition.as_bool().unwrap_or(false) {
        Ok("True".to_string())   // 返回要触发的输出 Pin 名称
    } else {
        Ok("False".to_string())
    }
}));
```

#### 步骤 4：设置元数据

```rust
let mut node = node;  // 转换为可变
node.set_metadata(
    vec!["Math".into(), "Operators".into()],  // 分类路径
    "math".into(),                            // UI 样式
    Some("Add two numbers".into())            // 描述（可选）
);
```

#### 步骤 5：注册节点

```rust
registry.register("add".into(), Arc::new(node));
```

---

## 完整示例

### 示例 1：简单的数学运算节点（Add）

```rust
use std::sync::Arc;
use crate::executor::node::registry::NodeRegistry;
use crate::executor::node::implementation::GenericNode;
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::value::{PinTypeDesc, ValueType};
use serde_json::Value;

pub fn register_add_node(registry: &NodeRegistry) {
    // 1. 创建节点原型
    let node = GenericNode::new_prototype("add", "Add (+)");
    
    // 2. 添加输入 Pin
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "A",
        PinTypeDesc::concrete(ValueType::Float64)
    ));
    
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "B",
        PinTypeDesc::concrete(ValueType::Float64)
    ));
    
    // 3. 添加输出 Pin
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Result",
        PinTypeDesc::concrete(ValueType::Float64)
    ));
    
    // 4. 设置数据处理器
    node.set_data_processor(Box::new(|ctx, node, _pin_id| {
        let a = ctx.get_pin_value(&node.inputs[0].id);
        let b = ctx.get_pin_value(&node.inputs[1].id);
        
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        
        Value::from(va + vb)
    }));
    
    // 5. 设置元数据
    let mut node = node;
    node.set_metadata(
        vec!["Math".into(), "Operators".into()],
        "math".into(),
        Some("Add two numbers together".into())
    );
    
    // 6. 注册节点
    registry.register("add".into(), Arc::new(node));
}
```

### 示例 2：带类型推断的泛型节点

```rust
use crate::executor::value::{TypeVarId, TypeConstraint};

pub fn register_generic_add_node(registry: &NodeRegistry) {
    let node = GenericNode::new_prototype("add", "Add (+)");
    
    // 创建类型变量（A、B、Result 共享同一类型）
    let type_var = TypeVarId::new();
    
    // 添加带约束的输入 Pin（只接受数值类型）
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "A",
        PinTypeDesc::type_var_with_constraints(
            type_var,
            vec![TypeConstraint::Numeric]
        )
    ));
    
    // 第二个输入使用相同的类型变量
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "B",
        PinTypeDesc::type_var(type_var)
    ));
    
    // 输出也使用相同的类型变量
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Result",
        PinTypeDesc::type_var(type_var)
    ));
    
    node.set_data_processor(Box::new(|ctx, node, _pin_id| {
        let a = ctx.get_pin_value(&node.inputs[0].id);
        let b = ctx.get_pin_value(&node.inputs[1].id);
        let va = a.as_f64().unwrap_or(0.0);
        let vb = b.as_f64().unwrap_or(0.0);
        Value::from(va + vb)
    }));
    
    let mut node = node;
    node.set_metadata(
        vec!["Math".into(), "Operators".into()],
        "math".into(),
        None
    );
    
    registry.register("add".into(), Arc::new(node));
}
```

### 示例 3：控制流节点（If-Else）

```rust
use crate::executor::pin::{GenericInExecPin, GenericOutExecPin};

pub fn register_if_else_node(registry: &NodeRegistry) {
    let node = GenericNode::new_prototype("if_else", "If-Else");
    
    // 添加输入执行 Pin
    node.add_in_exec_pin(GenericInExecPin::new(
        uuid::Uuid::nil(),
        "In"
    ));
    
    // 添加条件输入
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Condition",
        PinTypeDesc::concrete(ValueType::Boolean)
    ));
    
    // 添加输出执行 Pin
    node.add_out_exec_pin(GenericOutExecPin::new(
        uuid::Uuid::nil(),
        "True"
    ));
    
    node.add_out_exec_pin(GenericOutExecPin::new(
        uuid::Uuid::nil(),
        "False"
    ));
    
    // 设置控制流处理器
    node.set_flow_processor(Box::new(|ctx, node| {
        let condition = ctx.get_pin_value(&node.inputs[0].id);
        
        if condition.as_bool().unwrap_or(false) {
            Ok("True".to_string())
        } else {
            Ok("False".to_string())
        }
    }));
    
    let mut node = node;
    node.set_metadata(
        vec!["Control Flow".into()],
        "control".into(),
        Some("Branch execution based on condition".into())
    );
    
    registry.register("if_else".into(), Arc::new(node));
}
```

---

## 高级特性

### 1. 动态 Pin

动态 Pin 允许在运行时添加或删除 Pin。

**配置动态能力**：

```rust
use crate::executor::node::implementation::{
    NodeDynamicCapability, DynamicPinConfig, DynamicPinType, PinDirection
};

let node = GenericNode::new_prototype("dynamic_add", "Dynamic Add");

// 设置动态能力
node.set_dynamic_capability(NodeDynamicCapability {
    can_add_pins: true,
    dynamic_configs: vec![
        DynamicPinConfig {
            pin_type: DynamicPinType::Data,
            direction: PinDirection::Input,
            name_template: "Input {}".to_string(),
            data_type: PinTypeDesc::concrete(ValueType::Float64),
            min_count: 2,
            max_count: Some(10),
            can_reorder: true,
        }
    ],
    processor_generator: Some(Box::new(|node| {
        // 动态生成处理器
        Box::new(move |ctx, node_dto| {
            // 处理逻辑
            Ok("".to_string())
        })
    })),
});
```

**运行时添加 Pin**：

```rust
let config = DynamicPinConfig {
    pin_type: DynamicPinType::Data,
    direction: PinDirection::Input,
    name_template: "Input {}".to_string(),
    data_type: PinTypeDesc::concrete(ValueType::Float64),
    min_count: 0,
    max_count: None,
    can_reorder: true,
};

let pin_id = node.add_dynamic_pin(&config)?;
```

### 2. 类型推断系统

使用类型变量和约束实现泛型节点：

```rust
use crate::executor::value::{TypeVarId, TypeConstraint};

// 创建类型变量
let type_var = TypeVarId::new();

// 创建带约束的 Pin
let pin = GenericInDataPin::new(
    uuid::Uuid::nil(),
    "Input",
    PinTypeDesc::type_var_with_constraints(
        type_var,
        vec![
            TypeConstraint::Numeric,      // 只接受数值类型
            TypeConstraint::Comparable,   // 可比较
        ]
    )
);
```

**可用的类型约束**：

- `TypeConstraint::Numeric` - 数值类型（Int, Float）
- `TypeConstraint::Comparable` - 可比较类型
- `TypeConstraint::Iterable` - 可迭代类型（Array, List）
- `TypeConstraint::Serializable` - 可序列化类型
- `TypeConstraint::OneOf(types)` - 指定类型集合

### 3. 多输出节点

节点可以有多个输出 Pin，每个 Pin 可以独立计算：

```rust
node.set_data_processor(Box::new(|ctx, node, pin_id| {
    match pin_id {
        id if id == &node.outputs[0].id => {
            // 计算第一个输出
            Value::from(42)
        }
        id if id == &node.outputs[1].id => {
            // 计算第二个输出
            Value::from("result")
        }
        _ => Value::Null
    }
}));
```

---

## 最佳实践

### 1. 命名规范

- **节点类型**：使用小写下划线命名（如 `add`, `if_else`, `get_variable`）
- **节点标题**：使用友好的显示名称（如 "Add (+)", "If-Else", "Get Variable"）
- **Pin 名称**：使用大写驼峰或简短名称（如 "A", "B", "Result", "Condition"）

### 2. 类型安全

- 优先使用具体类型而非 `Any`
- 使用类型变量实现泛型节点
- 添加适当的类型约束

### 3. 错误处理

```rust
node.set_data_processor(Box::new(|ctx, node, _pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id);
    
    // 安全地处理类型转换
    match a.as_f64() {
        Some(value) => Value::from(value * 2.0),
        None => {
            // 记录错误或返回默认值
            Value::Null
        }
    }
}));
```

### 4. 性能优化

- 使用 `Arc` 共享不可变数据
- 避免在处理器中进行重复计算
- 合理使用缓存

### 5. 文档和注释

```rust
/// 创建加法节点
///
/// # 输入
/// - A: Float64 - 第一个加数
/// - B: Float64 - 第二个加数
///
/// # 输出
/// - Result: Float64 - 两数之和
///
/// # 示例
/// ```
/// A=5, B=3 => Result=8
/// ```
pub fn register_add_node(registry: &NodeRegistry) {
    // ...
}
```

### 6. 测试

为每个节点编写单元测试：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_add_node() {
        let registry = NodeRegistry::new();
        register_add_node(&registry);
        
        let proto = registry.get_prototype("add").unwrap();
        assert_eq!(proto.node_type(), "add");
        assert_eq!(proto.input_names().len(), 2);
        assert_eq!(proto.output_names().len(), 1);
    }
}
```

---

## 节点注册

所有内置节点在 `src-tauri/src/executor/node/catalog/mod.rs` 中注册：

```rust
pub fn register_builtin_nodes(registry: &NodeRegistry) {
    internal::register(registry);
    function::register(registry);
    control::register(registry);
    debug::register(registry);
    math::register(registry);
    variable::register(registry);
    data::register(registry);
    // ... 其他分类
}
```

创建新节点时，在相应的分类模块中添加注册函数。

---

## 相关文档

- [类型系统快速指南](src-tauri/docs/type-system/TYPE_SYSTEM_QUICK_GUIDE.md)
- [执行器设计](src-tauri/docs/architecture/EXECUTOR_DESIGN.md)
- [动态 Pin 快速入门](src-tauri/docs/examples/DYNAMIC_PIN_QUICKSTART.md)
- [Pin 值快速参考](src-tauri/docs/architecture/PIN_VALUE_QUICK_REFERENCE.md)

---

## 总结

创建一个新节点的核心步骤：

1. ✅ 创建 `GenericNode` 原型
2. ✅ 添加必要的 Pin（输入/输出，数据/执行）
3. ✅ 设置处理器（数据处理器或控制流处理器）
4. ✅ 设置元数据（分类、样式、描述）
5. ✅ 注册到 `NodeRegistry`
6. ✅ 编写测试

遵循这些步骤和最佳实践，你就可以创建功能完整、类型安全的节点了！
