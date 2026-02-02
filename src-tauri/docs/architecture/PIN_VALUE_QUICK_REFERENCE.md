# Pin 值系统快速参考

## 核心概念

### Pin 值的三层结构

```
┌─────────────────────────────────────┐
│  3. 连接值（最高优先级）              │
│     来自：连接的输出 Pin              │
│     特点：动态计算，不可编辑          │
└─────────────────────────────────────┘
              ↓ 如果未连接
┌─────────────────────────────────────┐
│  2. 用户值（中等优先级）              │
│     来自：用户在前端输入              │
│     特点：保存到项目文件              │
└─────────────────────────────────────┘
              ↓ 如果用户未设置
┌─────────────────────────────────────┐
│  1. 默认值（最低优先级）              │
│     来自：节点定义                   │
│     特点：通常不保存到项目文件        │
└─────────────────────────────────────┘
```

## 数据结构

### PinDto（最简版）

```rust
pub struct PinDto {
    pub id: String,
    pub name: String,
    pub pin_type: String,
    pub links: Vec<String>,
    
    // 默认值（节点定义）
    pub default_value: Option<Value>,
    
    // 🔑 用户值（用户设置）
    pub user_value: Option<Value>,
}
```

### 获取有效值

```rust
fn get_effective_value(pin: &PinDto) -> Value {
    if !pin.links.is_empty() {
        // 1. 有连接 → 使用连接值
        get_connected_value(pin)
    } else if let Some(user_val) = &pin.user_value {
        // 2. 用户设置了值 → 使用用户值
        user_val.clone()
    } else if let Some(default_val) = &pin.default_value {
        // 3. 有默认值 → 使用默认值
        default_val.clone()
    } else {
        // 4. 兜底 → 类型零值
        Value::Null
    }
}
```

## 前端实现

### 最简 Pin 组件

```typescript
function Pin({ pin, onValueChange }) {
  const isConnected = pin.links.length > 0;
  const value = pin.userValue ?? pin.defaultValue;
  
  return (
    <div className="pin">
      <div className="pin-dot" />
      <span>{pin.name}</span>
      
      {!isConnected && (
        <input
          type="number"
          value={value ?? 0}
          onChange={(e) => onValueChange(parseFloat(e.target.value))}
        />
      )}
    </div>
  );
}
```

### 更新值

```typescript
// 更新用户值
await invoke('update_pin_user_value', {
  subgraphId,
  nodeId,
  pinId,
  value: 5.0
});

// 清除用户值（恢复默认值）
await invoke('clear_pin_user_value', {
  subgraphId,
  nodeId,
  pinId
});
```

## 后端命令

### 更新 Pin 值

```rust
#[tauri::command]
pub fn update_pin_user_value(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
    value: Value,
) -> Result<(), String> {
    // 找到 Pin 并更新 user_value
    pin.user_value = Some(value);
    Ok(())
}
```

## 使用示例

### 示例 1：Add 节点

```rust
// 定义节点
let add = GenericNode::new_prototype("add", "Add");

// A 输入：默认值 0
add.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "A",
    PinTypeDesc::concrete(ValueType::Float64),
    Some(Value::from(0.0))  // 默认值
));
```

**前端显示**：
```
┌──────────────┐
│     Add      │
├──────────────┤
│ ○ A  [ 0.0 ] │  ← 可编辑
│ ○ B  [ 0.0 ] │
│      Sum  ○  │
└──────────────┘
```

**用户操作**：
```typescript
// 1. 用户修改 A 为 5.0
await updatePinValue(subgraphId, nodeId, pinA.id, 5.0);
// → pin.userValue = 5.0

// 2. 用户连接常量到 B
await connectPins(subgraphId, constPin.id, pinB.id);
// → pin.links = [constPin.id]
// → 输入框变灰

// 3. 执行
// A = 5.0 (用户值)
// B = 10.0 (连接值)
// Sum = 15.0
```

### 示例 2：Print 节点

```rust
// 定义节点
let print = GenericNode::new_prototype("print", "Print");

// Message 输入：默认值 "Hello"
print.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "Message",
    PinTypeDesc::concrete(ValueType::String),
    Some(Value::from("Hello"))
));
```

**前端显示**：
```
┌────────────────────┐
│       Print        │
├────────────────────┤
│ ○ Message [Hello ] │  ← 文本框
└────────────────────┘
```

## 项目文件格式

```json
{
  "nodes": [
    {
      "id": "node_1",
      "type": "add",
      "inputs": [
        {
          "id": "pin_1",
          "name": "A",
          "type": "float64",
          "links": [],
          "defaultValue": 0.0,
          "userValue": 5.0  // 🔑 用户设置的值
        },
        {
          "id": "pin_2",
          "name": "B",
          "type": "float64",
          "links": ["pin_3"],  // 已连接
          "defaultValue": 0.0,
          "userValue": null    // 连接时忽略
        }
      ]
    }
  ]
}
```

## 常见问题

### Q: 连接后用户值会丢失吗？

A: 不会。`userValue` 仍然保存，只是被连接值覆盖。断开连接后会恢复。

```typescript
// 连接前
pin.userValue = 5.0;
pin.links = [];
// 有效值 = 5.0

// 连接后
pin.userValue = 5.0;  // 仍然保存
pin.links = ["pin_x"];
// 有效值 = 连接值

// 断开连接
pin.userValue = 5.0;  // 恢复
pin.links = [];
// 有效值 = 5.0
```

### Q: 如何重置为默认值？

A: 清除 `userValue`。

```typescript
await invoke('clear_pin_user_value', {
  subgraphId,
  nodeId,
  pinId
});
// pin.userValue = null
// 有效值 = defaultValue
```

### Q: 默认值需要保存到项目文件吗？

A: 通常不需要。默认值来自节点定义，每次加载时从原型获取。只有 `userValue` 需要保存。

### Q: 如何实现不同的输入控件？

A: 添加 `widgetType` 字段。

```rust
pub struct PinDto {
    // ...
    pub widget_type: Option<String>,  // "slider", "color", "textarea"
}
```

```typescript
switch (pin.widgetType) {
  case 'slider':
    return <input type="range" ... />;
  case 'color':
    return <input type="color" ... />;
  default:
    return <input type="number" ... />;
}
```

## 完整文档

详细设计请参考：[Pin 默认值系统设计](./PIN_DEFAULT_VALUE_DESIGN.md)
