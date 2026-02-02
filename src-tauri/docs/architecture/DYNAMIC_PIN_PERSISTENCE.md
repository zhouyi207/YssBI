# 动态 Pin 持久化设计

## 问题描述

动态 Pin 的状态需要持久化，以支持：
1. **保存/加载项目**：重新打开项目时恢复动态 Pin
2. **复制/粘贴节点**：保留动态添加的 Pin
3. **撤销/重做**：正确恢复 Pin 状态

## 核心概念

### 节点的两种表示

1. **原型（Prototype）**：注册表中的模板
   - 定义节点的默认 Pin
   - 不包含动态添加的 Pin
   - 用于创建新节点

2. **实例（Instance）**：项目中的具体节点
   - 包含动态添加的 Pin
   - 保存在项目文件中
   - 每个实例可以有不同的 Pin 配置

## 设计方案

### 1. 扩展 SerializedNode

在项目文件中保存动态 Pin 信息：

```rust
// src/project/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedNode {
    pub id: String,
    pub node_type: String,
    pub title: String,
    pub position: Position,
    
    // 现有字段...
    pub inputs: Vec<SerializedPin>,
    pub outputs: Vec<SerializedPin>,
    
    // 🆕 新增：动态 Pin 元数据
    #[serde(rename = "dynamicPins", skip_serializing_if = "Option::is_none")]
    pub dynamic_pins: Option<Vec<DynamicPinMetadata>>,
}

/// 动态 Pin 元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DynamicPinMetadata {
    pub pin_id: String,
    pub pin_type: String,      // "Exec" 或 "Data"
    pub direction: String,     // "Input" 或 "Output"
    pub name: String,
    pub data_type: String,
    pub is_dynamic: bool,      // 标记为动态添加的
}
```

### 2. 保存流程

```
用户添加 Pin
    ↓
更新运行时节点（GenericNode）
    ↓
更新项目状态（SerializedNode）
    ↓
保存到文件
```

**实现**：

```rust
// commands/nodes.rs

#[tauri::command]
pub fn add_node_dynamic_pin(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_type: String,
    direction: String,
) -> Result<serde_json::Value, String> {
    // 1. 从项目状态获取节点
    let mut project = state.data.write().unwrap();
    let subgraph = get_subgraph_mut!(project, &subgraph_id)?;
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or("Node not found")?;
    
    // 2. 从注册表获取原型
    let registry = NodeRegistry::global();
    let prototype = registry.get_node(&node.node_type)?;
    
    // 3. 获取动态配置
    let pin_direction = match direction.as_str() {
        "input" => PinDirection::Input,
        "output" => PinDirection::Output,
        _ => return Err("Invalid direction".to_string()),
    };
    
    let config = prototype.get_dynamic_constraints(&pin_type, &pin_direction)
        .ok_or("Node does not support dynamic pins")?;
    
    // 4. 验证是否可以添加
    let current_count = match (&pin_direction, pin_type.as_str()) {
        (PinDirection::Input, "data") => node.inputs.len(),
        (PinDirection::Output, "data") => node.outputs.len(),
        _ => 0,
    };
    
    if let Some(max) = config.max_count {
        if current_count >= max {
            return Err(format!("Cannot add more pins: max={}", max));
        }
    }
    
    // 5. 生成新 Pin
    let pin_id = uuid::Uuid::new_v4();
    let pin_name = config.name_template.replace("{}", &(current_count + 1).to_string());
    
    let new_pin = SerializedPin {
        id: pin_id.to_string(),
        name: pin_name.clone(),
        pin_type: config.data_type.type_string(),
        links: vec![],
        default_value: None,
        is_array: false,
    };
    
    // 6. 添加到节点
    match pin_direction {
        PinDirection::Input => node.inputs.push(new_pin),
        PinDirection::Output => node.outputs.push(new_pin),
    }
    
    // 7. 记录动态 Pin 元数据
    let metadata = DynamicPinMetadata {
        pin_id: pin_id.to_string(),
        pin_type: format!("{:?}", config.pin_type),
        direction: format!("{:?}", pin_direction),
        name: pin_name.clone(),
        data_type: config.data_type.type_string(),
        is_dynamic: true,
    };
    
    if node.dynamic_pins.is_none() {
        node.dynamic_pins = Some(vec![]);
    }
    node.dynamic_pins.as_mut().unwrap().push(metadata);
    
    // 8. 返回结果
    Ok(serde_json::json!({
        "pinId": pin_id.to_string(),
        "name": pin_name,
        "type": format!("{:?}", config.pin_type),
        "direction": format!("{:?}", pin_direction)
    }))
}
```

### 3. 加载流程

```
加载项目文件
    ↓
解析 SerializedNode
    ↓
从注册表获取原型
    ↓
克隆原型创建实例
    ↓
恢复动态 Pin（如果有）
    ↓
创建运行时节点
```

**实现**：

```rust
// executor/context.rs

fn create_runtime_node_from_serialized(
    node_data: &SerializedNode,
    registry: &NodeRegistry,
) -> Result<GenericNode, String> {
    // 1. 从注册表获取原型
    let prototype = registry.get_node(&node_data.node_type)?;
    
    // 2. 克隆原型
    let runtime_node = prototype.clone_with_id(
        uuid::Uuid::parse_str(&node_data.id)?
    );
    
    // 3. 恢复动态 Pin（如果有）
    if let Some(dynamic_pins) = &node_data.dynamic_pins {
        for pin_meta in dynamic_pins {
            // 将元数据转换为 DynamicPinInfo
            let pin_info = DynamicPinInfo {
                pin_id: pin_meta.pin_id.clone(),
                pin_type: pin_meta.pin_type.clone(),
                direction: pin_meta.direction.clone(),
                name: pin_meta.name.clone(),
                data_type: pin_meta.data_type.clone(),
                is_dynamic: pin_meta.is_dynamic,
            };
            
            // 重建动态 Pin
            runtime_node.rebuild_from_dynamic_info(vec![pin_info])?;
        }
    }
    
    Ok(runtime_node)
}
```

### 4. 复制/粘贴流程

```
用户复制节点
    ↓
序列化节点（包含 dynamic_pins）
    ↓
用户粘贴
    ↓
生成新 ID
    ↓
保留 dynamic_pins 元数据
    ↓
创建新节点实例
```

**实现**：

```rust
// commands/nodes.rs

#[tauri::command]
pub fn duplicate_node(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    new_position: Position,
) -> Result<SerializedNode, String> {
    let mut project = state.data.write().unwrap();
    let subgraph = get_subgraph_mut!(project, &subgraph_id)?;
    
    // 1. 找到源节点
    let source_node = subgraph.nodes.iter()
        .find(|n| n.id == node_id)
        .ok_or("Node not found")?
        .clone();
    
    // 2. 创建新节点（保留动态 Pin）
    let new_node = SerializedNode {
        id: uuid::Uuid::new_v4().to_string(),
        position: new_position,
        // 复制所有字段，包括 inputs, outputs, dynamic_pins
        ..source_node
    };
    
    // 3. 重新生成 Pin ID（避免冲突）
    let new_node = regenerate_pin_ids(new_node);
    
    // 4. 添加到子图
    subgraph.nodes.push(new_node.clone());
    
    Ok(new_node)
}

fn regenerate_pin_ids(mut node: SerializedNode) -> SerializedNode {
    // 为所有 Pin 生成新 ID
    for input in &mut node.inputs {
        input.id = uuid::Uuid::new_v4().to_string();
        input.links.clear(); // 清除连接
    }
    for output in &mut node.outputs {
        output.id = uuid::Uuid::new_v4().to_string();
        output.links.clear();
    }
    
    // 更新动态 Pin 元数据中的 ID
    if let Some(dynamic_pins) = &mut node.dynamic_pins {
        for (i, pin_meta) in dynamic_pins.iter_mut().enumerate() {
            if pin_meta.direction == "Input" && i < node.inputs.len() {
                pin_meta.pin_id = node.inputs[i].id.clone();
            } else if pin_meta.direction == "Output" {
                let output_index = i - node.inputs.len();
                if output_index < node.outputs.len() {
                    pin_meta.pin_id = node.outputs[output_index].id.clone();
                }
            }
        }
    }
    
    node
}
```

## 数据流图

### 创建新节点

```
注册表原型 (2 inputs)
    ↓
克隆
    ↓
新节点实例 (2 inputs)
    ↓
保存到项目
```

### 添加动态 Pin

```
节点实例 (2 inputs)
    ↓
添加 Pin
    ↓
节点实例 (3 inputs)
    ↓
更新 dynamic_pins 元数据
    ↓
保存到项目
```

### 复制节点

```
源节点 (3 inputs + dynamic_pins)
    ↓
克隆
    ↓
新节点 (3 inputs + dynamic_pins)
    ↓
重新生成 ID
    ↓
保存到项目
```

### 加载项目

```
项目文件
    ↓
解析 SerializedNode (3 inputs + dynamic_pins)
    ↓
从注册表获取原型 (2 inputs)
    ↓
克隆原型
    ↓
应用 dynamic_pins 元数据
    ↓
运行时节点 (3 inputs)
```

## 前端行为

### 创建节点

```typescript
// 创建新节点 - 使用默认 Pin
const node = await createNode(subgraphId, {
  type: 'dynamic_add',
  position: { x: 100, y: 100 }
});

console.log(node.inputs.length); // 2 (默认)
console.log(node.dynamicPins);   // undefined
```

### 添加 Pin

```typescript
// 添加 Pin
await addNodeDynamicPin(subgraphId, node.id, 'data', 'input');

// 重新获取节点
const updatedNode = await getNode(subgraphId, node.id);
console.log(updatedNode.inputs.length); // 3
console.log(updatedNode.dynamicPins);   // [{ pinId: "...", ... }]
```

### 复制节点

```typescript
// 复制节点 - 保留动态 Pin
const newNode = await duplicateNode(
  subgraphId,
  node.id,
  { x: 200, y: 100 }
);

console.log(newNode.inputs.length); // 3 (保留)
console.log(newNode.dynamicPins);   // [{ pinId: "...", ... }] (保留)
```

### 保存/加载

```typescript
// 保存项目
await saveProject(projectPath);

// 关闭并重新打开

// 加载项目
await loadProject(projectPath);

// 节点恢复，包括动态 Pin
const node = await getNode(subgraphId, nodeId);
console.log(node.inputs.length); // 3 (恢复)
console.log(node.dynamicPins);   // [{ pinId: "...", ... }] (恢复)
```

## 实现步骤

### Phase 1: 数据结构 ✅

- [x] 定义 `DynamicPinMetadata`
- [x] 扩展 `SerializedNode`

### Phase 2: 持久化

- [ ] 实现 `add_node_dynamic_pin` 命令
- [ ] 实现 `remove_node_dynamic_pin` 命令
- [ ] 更新项目保存逻辑

### Phase 3: 加载

- [ ] 实现 `create_runtime_node_from_serialized`
- [ ] 实现 `rebuild_from_dynamic_info`
- [ ] 更新项目加载逻辑

### Phase 4: 复制/粘贴

- [ ] 实现 `duplicate_node` 命令
- [ ] 实现 `regenerate_pin_ids`

### Phase 5: 前端集成

- [ ] 更新节点序列化/反序列化
- [ ] 实现复制/粘贴逻辑
- [ ] 测试保存/加载

## 注意事项

1. **Pin ID 唯一性**：复制节点时必须重新生成 Pin ID
2. **连接清除**：复制节点时清除所有连接
3. **向后兼容**：旧项目文件没有 `dynamic_pins` 字段，需要兼容处理
4. **类型安全**：确保动态 Pin 的类型与原型配置一致
5. **验证**：添加/移除 Pin 时验证约束（min/max count）

## 测试用例

```rust
#[test]
fn test_save_and_load_dynamic_pins() {
    // 1. 创建节点并添加动态 Pin
    let node = create_node("dynamic_add");
    add_dynamic_pin(&node, "data", "input");
    
    // 2. 保存到文件
    save_project(&project);
    
    // 3. 加载项目
    let loaded_project = load_project(&path);
    let loaded_node = get_node(&loaded_project, node.id);
    
    // 4. 验证动态 Pin 恢复
    assert_eq!(loaded_node.inputs.len(), 3);
    assert!(loaded_node.dynamic_pins.is_some());
}

#[test]
fn test_duplicate_node_with_dynamic_pins() {
    // 1. 创建节点并添加动态 Pin
    let node = create_node("dynamic_add");
    add_dynamic_pin(&node, "data", "input");
    
    // 2. 复制节点
    let new_node = duplicate_node(&node);
    
    // 3. 验证
    assert_eq!(new_node.inputs.len(), 3);
    assert_ne!(new_node.id, node.id);
    assert_ne!(new_node.inputs[0].id, node.inputs[0].id);
}
```

## 相关文档

- [动态 Pin 总结](../examples/DYNAMIC_PIN_SUMMARY.md)
- [前端集成指南](../examples/DYNAMIC_PIN_FRONTEND_GUIDE.md)
