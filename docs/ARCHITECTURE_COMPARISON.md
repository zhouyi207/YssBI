# 架构对比：旧 vs 新

## 1. 节点创建对比

### 旧架构（违反规则）
```rust
// 问题：通过 index 访问 Pin
node.set_data_processor(Box::new(|ctx, node, _pin_id| {
    let a = ctx.get_pin_value(&node.inputs[0].id);  // ❌ 使用 index
    let b = ctx.get_pin_value(&node.inputs[1].id);  // ❌ 使用 index
    
    let va = a.as_f64().unwrap_or(0.0);
    let vb = b.as_f64().unwrap_or(0.0);
    
    Value::from(va + vb)
}));
```

### 新架构（符合规则）
```rust
// 正确：通过语义角色访问
NodeDefinition::new("add", "Add (+)")
    .with_processor(Arc::new(|ctx| {
        let a = ctx.get_input_by_role(&PinRole::Custom("A".into()))?;  // ✅ 使用角色
        let b = ctx.get_input_by_role(&PinRole::Custom("B".into()))?;  // ✅ 使用角色
        
        let result = match (a, b) {
            (DataValue::Float64(va), DataValue::Float64(vb)) => va + vb,
            _ => return Err("Invalid types".into()),
        };
        
        ctx.emit_output_by_role(&PinRole::Result, DataValue::Float64(result))?;
        Ok(PinRole::ExecOut)
    }))
```

## 2. 动态 Pin 对比

### 旧架构
```rust
// 问题：遍历所有输入，依赖顺序
for input in node.inputs() {
    let value = ctx.get_pin_value(&input.id());
    // 处理...
}
```

### 新架构
```rust
// 正确：通过角色获取动态组
let operands = ctx.get_inputs_by_role(&PinRole::Operands)?;
for operand in operands {
    // 处理...
}
```

## 3. 连接管理对比

### 旧架构（违反规则）
```rust
// 问题：Pin 内部存储连接
pub struct GenericInDataPin {
    upstream: RwLock<Option<PinId>>,  // ❌ Pin 持有连接
}

pub struct GenericOutDataPin {
    downstream: RwLock<Vec<PinId>>,  // ❌ Pin 持有连接
}
```

### 新架构（符合规则）
```rust
// 正确：Graph 统一管理连接
pub struct PinInstance {
    // 不包含任何连接字段  // ✅
}

pub struct Graph {
    connections: HashMap<PinId, Vec<PinId>>,  // ✅ 唯一真实来源
}
```

## 4. 节点定义对比

### 旧架构（混合）
```rust
// 问题：定义和实例混合
pub struct GenericNode {
    id: NodeId,                    // 运行时
    node_type: String,             // 定义
    in_data_pins: DashMap<...>,    // 运行时  ❌
    flow_processor: Mutex<...>,    // 定义
}
```

### 新架构（分离）
```rust
// 正确：定义（静态）
pub struct NodeDefinition {
    node_type: String,
    pins: Vec<PinDefinition>,
    processor: Option<NodeProcessor>,
    // 不包含运行时状态  ✅
}

// 正确：实例（运行时）
pub struct NodeInstance {
    id: NodeId,
    definition_type: String,
    // 不持有 Pin  ✅
}
```

## 5. 执行上下文对比

### 旧架构
```rust
// 问题：直接访问节点内部结构
pub trait ExecutionContextTrait {
    fn get_pin_value(&self, pin_id: &PinId) -> Value;
}

// 使用时需要知道 Pin ID
let value = ctx.get_pin_value(&node.inputs[0].id);  // ❌
```

### 新架构
```rust
// 正确：基于语义的 API
pub trait NodeExecutionContext {
    fn get_input_by_role(&self, role: &PinRole) -> Result<DataValue, String>;
    fn get_inputs_by_role(&self, role: &PinRole) -> Result<Vec<DataValue>, String>;
    fn emit_output_by_role(&mut self, role: &PinRole, value: DataValue) -> Result<(), String>;
}

// 使用时只需要知道语义角色
let value = ctx.get_input_by_role(&PinRole::Condition)?;  // ✅
```

## 6. If-Else 节点对比

### 旧架构
```rust
node.set_flow_processor(Box::new(|ctx, node| {
    let condition = ctx.get_pin_value(&node.inputs[0].id);  // ❌ index
    
    if condition.as_bool().unwrap_or(false) {
        Ok("True".to_string())  // ❌ 返回名称
    } else {
        Ok("False".to_string())
    }
}));
```

### 新架构
```rust
NodeDefinition::new("if_else", "If-Else")
    .add_pin(PinDefinition::exec_input(PinRole::ExecIn, "In"))
    .add_pin(PinDefinition::data_input(PinRole::Condition, "Condition", ...))
    .add_pin(PinDefinition::exec_output(PinRole::ExecTrue, "True"))
    .add_pin(PinDefinition::exec_output(PinRole::ExecFalse, "False"))
    .with_processor(Arc::new(|ctx| {
        let condition = ctx.get_input_by_role(&PinRole::Condition)?;  // ✅ 角色
        
        match condition {
            DataValue::Boolean(true) => Ok(PinRole::ExecTrue),  // ✅ 返回角色
            DataValue::Boolean(false) => Ok(PinRole::ExecFalse),
            _ => Err("Invalid condition".into()),
        }
    }))
```

## 7. Sequence 节点对比

### 旧架构
```rust
// 问题：依赖 Pin 名称和顺序
node.set_flow_processor(Box::new(|ctx, node| {
    // 遍历所有输出执行 Pin
    for exec_pin in node.out_exec_pins.iter() {  // ❌ 直接访问
        // 触发...
    }
    Ok("".into())
}));
```

### 新架构
```rust
// 正确：通过角色访问动态组
NodeDefinition::new("sequence", "Sequence")
    .add_pin(PinDefinition::exec_input(PinRole::ExecIn, "In"))
    .add_pin(PinDefinition::dynamic_group(
        PinRole::Steps,  // ✅ 动态组角色
        PinDirection::Output,
        PinKind::Exec,
        None,
        "steps",
    ))
    .with_processor(Arc::new(|ctx| {
        // 返回 Steps 角色，执行器会处理所有 Steps
        Ok(PinRole::Steps)  // ✅
    }))
```

## 关键差异总结

| 方面 | 旧架构 | 新架构 |
|------|--------|--------|
| Pin 访问 | index/name | PinRole（语义角色） |
| 连接管理 | Pin 内部 | Graph 统一管理 |
| 节点结构 | 定义+实例混合 | 定义与实例分离 |
| 运行时状态 | Node 持有 | Graph 持有 |
| 处理器 API | 直接访问 Pin | 基于角色的上下文 API |
| 动态 Pin | 遍历所有 Pin | 通过角色组访问 |

