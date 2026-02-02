# 动态 Pin 示例：多输入 Add 节点

## 需求
实现一个支持动态添加输入的 Add 节点，可以计算任意数量的数字相加。

## 实现步骤

### 1. 创建支持动态 Pin 的 Add 节点

```rust
use crate::executor::node::implementation::{
    GenericNode, DynamicPinConfig, DynamicPinType, PinDirection, 
    NodeDynamicCapability, ProcessorGenerator
};
use crate::executor::pin::{GenericInDataPin, GenericOutDataPin};
use crate::executor::value::{PinTypeDesc, TypeConstraint};

pub fn create_dynamic_add_node() -> GenericNode {
    let node = GenericNode::new_prototype("dynamic_add", "Add (Dynamic)");
    
    // 1. 添加初始的两个输入 Pin（最小数量）
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Input 1",
        PinTypeDesc::concrete_with_constraints(
            crate::executor::value::ValueType::Float,
            vec![TypeConstraint::Numeric]
        )
    ));
    
    node.add_in_data_pin(GenericInDataPin::new(
        uuid::Uuid::nil(),
        "Input 2",
        PinTypeDesc::concrete_with_constraints(
            crate::executor::value::ValueType::Float,
            vec![TypeConstraint::Numeric]
        )
    ));
    
    // 2. 添加输出 Pin
    node.add_out_data_pin(GenericOutDataPin::new(
        uuid::Uuid::nil(),
        "Sum",
        PinTypeDesc::concrete(crate::executor::value::ValueType::Float)
    ));
    
    // 3. 配置动态能力
    let dynamic_config = DynamicPinConfig {
        pin_type: DynamicPinType::Data,
        direction: PinDirection::Input,
        name_template: "Input {}".to_string(),  // 生成 "Input 3", "Input 4" ...
        data_type: PinTypeDesc::concrete_with_constraints(
            crate::executor::value::ValueType::Float,
            vec![TypeConstraint::Numeric]
        ),
        min_count: 2,           // 最少 2 个输入
        max_count: Some(10),    // 最多 10 个输入
        can_reorder: true,      // 允许重新排序
    };
    
    // 4. 创建处理器生成器（根据当前 Pin 数量生成处理器）
    let processor_generator: ProcessorGenerator = Box::new(|node: &GenericNode| {
        // 获取所有输入 Pin 的数量
        let input_count = node.inputs().len();
        
        Box::new(move |ctx, node_dto, _pin_id| {
            let mut sum = 0.0;
            
            // 遍历所有输入 Pin，累加值
            for i in 0..input_count {
                if i < node_dto.inputs.len() {
                    let value = ctx.get_pin_value(&node_dto.inputs[i].id);
                    sum += value.as_f64().unwrap_or(0.0);
                }
            }
            
            Value::from(sum)
        })
    });
    
    // 5. 设置动态能力
    let capability = NodeDynamicCapability {
        can_add_pins: true,
        dynamic_configs: vec![dynamic_config],
        processor_generator: Some(processor_generator),
    };
    
    node.set_dynamic_capability(capability);
    
    // 6. 初始化处理器
    node.regenerate_processor().unwrap();
    
    // 7. 设置元数据
    let mut node = node;
    node.set_metadata(
        vec!["Math".into(), "Dynamic".into()],
        "math".into(),
        Some("Add multiple numbers together".into())
    );
    
    node
}
```

### 2. 注册节点

```rust
// 在 catalog/math/mod.rs 中
pub fn register_dynamic_nodes(registry: &NodeRegistry) {
    let dynamic_add = create_dynamic_add_node();
    registry.register("dynamic_add".into(), Arc::new(dynamic_add));
}
```

### 3. 前端使用

#### 3.1 创建节点
```typescript
// 创建一个动态 Add 节点
const node = await createNode(subgraphId, {
  type: 'dynamic_add',
  title: 'Add (Dynamic)',
  position: { x: 100, y: 100 }
});
```

#### 3.2 添加输入 Pin
```typescript
// 添加第 3 个输入
const result = await addNodeDynamicPin(
  subgraphId,
  node.id,
  'data',      // pin_type
  'input'      // direction
);

// 现在节点有 3 个输入：Input 1, Input 2, Input 3
```

#### 3.3 连接和执行
```typescript
// 连接常量节点到动态 Add 节点
await connectPins(subgraphId, const1.outputs[0].id, node.inputs[0].id);
await connectPins(subgraphId, const2.outputs[0].id, node.inputs[1].id);
await connectPins(subgraphId, const3.outputs[0].id, node.inputs[2].id);

// 执行图
const result = await executeGraph(subgraphId);
// 结果：Sum = const1 + const2 + const3
```

#### 3.4 移除输入 Pin
```typescript
// 移除第 3 个输入（如果不再需要）
await removeNodeDynamicPin(subgraphId, node.id, node.inputs[2].id);
```

## 工作流程

```
1. 用户创建 dynamic_add 节点
   ↓
2. 节点初始有 2 个输入 Pin（Input 1, Input 2）
   ↓
3. 用户点击 "+" 按钮添加输入
   ↓
4. 调用 add_node_dynamic_pin()
   ↓
5. 后端验证：
   - 是否支持动态 Pin？
   - 是否达到最大数量？
   ↓
6. 生成新 Pin（Input 3）
   ↓
7. 重新生成处理器（现在处理 3 个输入）
   ↓
8. 返回更新后的节点定义
   ↓
9. 前端更新 UI，显示新的 Pin
   ↓
10. 用户连接数据并执行
```

## 关键点

### 1. 处理器动态生成
```rust
// ❌ 错误：固定处理 2 个输入
node.set_data_processor(Box::new(|ctx, node, _| {
    let a = ctx.get_pin_value(&node.inputs[0].id);
    let b = ctx.get_pin_value(&node.inputs[1].id);
    Value::from(a.as_f64().unwrap_or(0.0) + b.as_f64().unwrap_or(0.0))
}));

// ✅ 正确：根据当前 Pin 数量动态处理
let processor_generator = Box::new(|node: &GenericNode| {
    let input_count = node.inputs().len();
    Box::new(move |ctx, node_dto, _| {
        let mut sum = 0.0;
        for i in 0..input_count {
            sum += ctx.get_pin_value(&node_dto.inputs[i].id)
                .as_f64().unwrap_or(0.0);
        }
        Value::from(sum)
    })
});
```

### 2. 约束配置
```rust
DynamicPinConfig {
    min_count: 2,        // 至少 2 个输入（保证有意义）
    max_count: Some(10), // 最多 10 个（避免性能问题）
    can_reorder: true,   // 允许调整输入顺序
    // ...
}
```

### 3. 名称模板
```rust
name_template: "Input {}".to_string()
// 生成：Input 1, Input 2, Input 3, ...

name_template: "Value {}".to_string()
// 生成：Value 1, Value 2, Value 3, ...
```

## 完整示例：前端 UI

```typescript
// DynamicAddNode.tsx
function DynamicAddNode({ node, onAddPin, onRemovePin }) {
  const canAddMore = node.inputs.length < 10;
  const canRemove = node.inputs.length > 2;
  
  return (
    <div className="node">
      <div className="node-header">Add (Dynamic)</div>
      
      <div className="node-inputs">
        {node.inputs.map((input, index) => (
          <div key={input.id} className="pin-row">
            <Pin pin={input} />
            {canRemove && index >= 2 && (
              <button onClick={() => onRemovePin(input.id)}>
                ✕
              </button>
            )}
          </div>
        ))}
        
        {canAddMore && (
          <button onClick={onAddPin} className="add-pin-btn">
            + Add Input
          </button>
        )}
      </div>
      
      <div className="node-outputs">
        <Pin pin={node.outputs[0]} />
      </div>
    </div>
  );
}
```

## 其他应用场景

### 1. 字符串拼接（多输入）
```rust
name_template: "String {}".to_string()
// 拼接任意数量的字符串
```

### 2. Switch 节点（多输出）
```rust
DynamicPinConfig {
    pin_type: DynamicPinType::Exec,
    direction: PinDirection::Output,
    name_template: "Case {}".to_string(),
    // 根据输入值选择不同的输出执行
}
```

### 3. 数组构造器（多输入）
```rust
name_template: "Element {}".to_string()
// 从多个输入构造数组
```

### 4. 函数调用（多参数）
```rust
name_template: "Arg {}".to_string()
// 动态参数数量的函数调用
```

## 优势

1. **灵活性**：用户可以根据需要添加/移除输入
2. **类型安全**：每个 Pin 都有类型约束
3. **性能**：处理器在 Pin 变更时重新生成，执行时无额外开销
4. **用户体验**：直观的 UI，无需预定义固定数量的输入

## 注意事项

1. **最小/最大限制**：防止用户创建无意义或性能问题的节点
2. **处理器重新生成**：每次添加/移除 Pin 都会重新生成处理器
3. **序列化**：动态 Pin 信息需要保存到项目文件中
4. **连接验证**：移除 Pin 时需要断开相关连接
