# 动态 Pin 功能总结

## 概述

动态 Pin 允许节点在运行时动态添加或移除 Pin，提供更灵活的节点设计。

## 实现位置

当前动态 Pin 的实现在 `src/executor/node/implementation.rs` 中，包含：
- 数据结构定义（DynamicPinType, PinDirection, DynamicPinConfig 等）
- GenericNode 的动态 Pin 方法
- 约 400 行代码

**建议重构**：将动态 Pin 相关代码移到独立的 `dynamic_pins.rs` 模块。

## 核心概念

### 1. DynamicPinConfig
配置动态 Pin 的行为：
```rust
DynamicPinConfig {
    pin_type: DynamicPinType::Data,      // Exec 或 Data
    direction: PinDirection::Input,       // Input 或 Output
    name_template: "Input {}",            // 名称模板
    data_type: PinTypeDesc::unknown(),    // Pin 类型
    min_count: 2,                         // 最小数量
    max_count: Some(10),                  // 最大数量
    can_reorder: true,                    // 是否可重排序
}
```

### 2. NodeDynamicCapability
节点的动态能力描述：
```rust
NodeDynamicCapability {
    can_add_pins: true,                   // 是否支持动态 Pin
    dynamic_configs: vec![config],        // 动态配置列表
    processor_generator: Some(generator), // 处理器生成器（可选）
}
```

### 3. ProcessorGenerator
用于流程节点，在 Pin 变更时重新生成处理器：
```rust
let generator: ProcessorGenerator = Box::new(|node: &GenericNode| {
    // 根据当前 Pin 状态生成新的处理器
    Box::new(move |ctx, node_dto| {
        // 处理逻辑
    })
});
```

## 使用场景

### 场景 1：数据节点（不需要 processor_generator）

**示例：动态 Add 节点**

```rust
// 数据处理器自动遍历所有输入
node.set_data_processor(Box::new(|ctx, node_dto, _pin_id| {
    let mut sum = 0.0;
    for input in &node_dto.inputs {
        sum += ctx.get_pin_value(&input.id).as_f64().unwrap_or(0.0);
    }
    Value::from(sum)
}));

// 不需要 processor_generator
let capability = NodeDynamicCapability {
    can_add_pins: true,
    dynamic_configs: vec![config],
    processor_generator: None,  // ✅ 数据节点不需要
};
```

**优点**：
- 简单直接
- 处理器自动适应 Pin 数量
- 无需重新生成

**适用于**：
- 数学运算（Add, Multiply, Max, Min）
- 字符串拼接
- 数组构造
- 任何输入数量不影响处理逻辑的节点

### 场景 2：流程节点（需要 processor_generator）

**示例：动态 Switch 节点**

```rust
let processor_generator: ProcessorGenerator = Box::new(|node: &GenericNode| {
    // 获取当前输出 Pin 的名称
    let output_names = node.get_dynamic_exec_output_names();
    
    Box::new(move |ctx, node_dto| {
        // 根据输入值选择输出
        let index = ctx.get_pin_value(&node_dto.inputs[0].id)
            .as_i64().unwrap_or(0) as usize;
        
        if index < output_names.len() {
            Ok(output_names[index].clone())
        } else {
            Ok("Default".to_string())
        }
    })
});

let capability = NodeDynamicCapability {
    can_add_pins: true,
    dynamic_configs: vec![config],
    processor_generator: Some(processor_generator),  // ✅ 流程节点需要
};
```

**优点**：
- 处理器根据当前 Pin 状态生成
- 支持复杂的流程控制

**适用于**：
- Switch/Case 节点
- MultiGate 节点
- 动态路由节点
- 任何输出 Pin 数量影响执行逻辑的节点

## API 使用

### 创建支持动态 Pin 的节点

```rust
// 1. 创建节点原型
let node = GenericNode::new_prototype("dynamic_add", "Add (Dynamic)");

// 2. 添加初始 Pin
node.add_in_data_pin(...);
node.add_out_data_pin(...);

// 3. 设置处理器
node.set_data_processor(...);  // 或 set_flow_processor

// 4. 配置动态能力
let config = DynamicPinConfig { ... };
let capability = NodeDynamicCapability { ... };
node.set_dynamic_capability(capability);

// 5. 设置元数据
node.set_metadata(...);
```

### 运行时操作

```rust
// 添加 Pin
let pin_id = node.add_dynamic_pin(&config)?;

// 移除 Pin
node.remove_dynamic_pin(pin_id)?;

// 检查是否支持
if node.supports_dynamic_pins() {
    // ...
}

// 获取约束
let config = node.get_dynamic_constraints("data", &PinDirection::Input);
```

## 前端集成

### Tauri 命令

```rust
// 获取动态约束
get_node_dynamic_constraints(subgraph_id, node_id)

// 添加 Pin
add_node_dynamic_pin(subgraph_id, node_id, pin_type, direction)

// 移除 Pin
remove_node_dynamic_pin(subgraph_id, node_id, pin_id)

// 验证操作
validate_pin_operation(subgraph_id, node_id, operation)
```

### TypeScript 示例

```typescript
// 添加输入
const result = await addNodeDynamicPin(
  subgraphId,
  nodeId,
  'data',    // 'data' 或 'exec'
  'input'    // 'input' 或 'output'
);

// 移除输入
await removeNodeDynamicPin(subgraphId, nodeId, pinId);
```

## 实现的节点示例

### 1. 动态 Add（已实现）
- 位置：`src/executor/node/catalog/math/dynamic_add.rs`
- 类型：数据节点
- 输入：2-10 个数字
- 输出：总和

### 2. 其他可实现的节点

**数据节点**：
- Dynamic Multiply：多个数字相乘
- Dynamic Concat：多个字符串拼接
- Dynamic Array：构造任意长度数组
- Dynamic Max/Min：找出多个值的最大/最小值

**流程节点**：
- Dynamic Switch：根据值选择多个输出之一
- Dynamic Sequence：按顺序执行多个输出
- Dynamic MultiGate：循环执行多个输出

## 注意事项

### 1. 性能考虑
- 设置合理的 `max_count`（建议 10-20）
- 避免在循环中频繁添加/移除 Pin

### 2. 类型安全
- 使用 `TypeConstraint` 确保类型正确
- 动态添加的 Pin 继承配置的类型

### 3. 序列化
- 动态 Pin 信息需要保存到项目文件
- 使用 `get_dynamic_pin_info()` 和 `rebuild_from_dynamic_info()`

### 4. 连接管理
- 移除 Pin 时需要断开相关连接
- 前端需要处理 Pin 变更事件

## 测试

```rust
#[test]
fn test_dynamic_add() {
    let node = create_dynamic_add_node();
    
    // 验证初始状态
    assert_eq!(node.input_names().len(), 2);
    assert!(node.supports_dynamic_pins());
    
    // 添加 Pin
    let config = node.get_dynamic_constraints("data", &PinDirection::Input).unwrap();
    let pin_id = node.add_dynamic_pin(&config).unwrap();
    assert_eq!(node.input_names().len(), 3);
    
    // 移除 Pin
    node.remove_dynamic_pin(pin_id).unwrap();
    assert_eq!(node.input_names().len(), 2);
}
```

## 未来改进

1. **模块化**：将动态 Pin 代码移到 `dynamic_pins.rs`
2. **UI 增强**：拖拽重排序、批量添加
3. **模板系统**：预定义常用的动态节点模板
4. **约束验证**：更强的类型检查和约束验证
5. **撤销/重做**：支持 Pin 操作的撤销

## 相关文档

- [动态 Pin Add 示例](./DYNAMIC_PIN_ADD_EXAMPLE.md)
- [执行器设计](../architecture/EXECUTOR_DESIGN.md)
- [类型系统快速指南](../type-system/TYPE_SYSTEM_QUICK_GUIDE.md)
