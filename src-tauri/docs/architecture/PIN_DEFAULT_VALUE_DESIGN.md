# Pin 默认值系统设计

## 需求

1. **默认值定义**：节点定义时可以指定 Pin 的默认值
2. **前端编辑**：用户可以在前端修改 Pin 的默认值
3. **值优先级**：连接线的值 > 用户设置的值 > 节点定义的默认值
4. **持久化**：用户修改的默认值保存到项目文件

## 核心概念

### Pin 值的三个来源

```
1. 节点定义的默认值（Prototype Default）
   ↓ 可被覆盖
2. 用户设置的值（User Override）
   ↓ 可被覆盖
3. 连接线的值（Connected Value）
```

### 优先级规则

```rust
fn get_pin_value(pin: &Pin) -> Value {
    if pin.is_connected() {
        // 1. 最高优先级：连接线的值
        return get_connected_value(pin);
    } else if let Some(user_value) = pin.user_value {
        // 2. 中等优先级：用户设置的值
        return user_value;
    } else if let Some(default_value) = pin.default_value {
        // 3. 最低优先级：节点定义的默认值
        return default_value;
    } else {
        // 4. 兜底：类型的零值
        return get_zero_value(pin.pin_type);
    }
}
```

## 数据结构设计

### 1. PinDto（已有，需要扩展）

```rust
// src/project/dto.rs

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PinDto {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    pub links: Vec<String>,
    
    // 🔑 默认值（来自节点定义）
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
    
    // 🆕 用户设置的值（覆盖默认值）
    #[serde(rename = "userValue", skip_serializing_if = "Option::is_none")]
    pub user_value: Option<Value>,
    
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
    
    // 🆕 是否显示为输入控件（前端使用）
    #[serde(rename = "showWidget", default = "default_true")]
    pub show_widget: bool,
    
    // 🆕 控件类型提示（前端使用）
    #[serde(rename = "widgetType", skip_serializing_if = "Option::is_none")]
    pub widget_type: Option<String>,
}

fn default_true() -> bool {
    true
}
```

### 2. SerializedPin（项目文件）

```rust
// src/project/mod.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedPin {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub pin_type: String,
    #[serde(default)]
    pub links: Vec<String>,
    
    // 默认值（来自节点定义，通常不保存）
    #[serde(rename = "defaultValue", skip_serializing_if = "Option::is_none")]
    pub default_value: Option<Value>,
    
    // 🔑 用户设置的值（需要保存）
    #[serde(rename = "userValue", skip_serializing_if = "Option::is_none")]
    pub user_value: Option<Value>,
    
    #[serde(rename = "isArray", default)]
    pub is_array: bool,
}
```

### 3. GenericInDataPin（运行时）

```rust
// src/executor/pin/implementation.rs

pub struct GenericInDataPin {
    id: PinId,
    name: String,
    data_type: PinTypeDesc,
    
    // 默认值（来自节点定义）
    default_value: Option<Value>,
    
    // 🆕 用户设置的值
    user_value: RwLock<Option<Value>>,
    
    // 连接状态
    connected_pin: RwLock<Option<PinId>>,
    
    // ...
}

impl GenericInDataPin {
    /// 获取 Pin 的有效值
    pub fn get_effective_value(&self, ctx: &dyn ExecutionContextTrait) -> Value {
        // 1. 如果有连接，使用连接的值
        if let Some(connected_id) = self.connected_pin.read().unwrap().as_ref() {
            return ctx.get_pin_value(&connected_id.to_string());
        }
        
        // 2. 如果用户设置了值，使用用户值
        if let Some(user_val) = self.user_value.read().unwrap().as_ref() {
            return user_val.clone();
        }
        
        // 3. 使用默认值
        if let Some(default_val) = &self.default_value {
            return default_val.clone();
        }
        
        // 4. 返回类型的零值
        self.data_type.get_zero_value()
    }
    
    /// 设置用户值
    pub fn set_user_value(&self, value: Option<Value>) {
        *self.user_value.write().unwrap() = value;
    }
    
    /// 获取用户值
    pub fn get_user_value(&self) -> Option<Value> {
        self.user_value.read().unwrap().clone()
    }
}
```

## 使用场景

### 场景 1：数字常量输入

```rust
// 节点定义
let add_node = GenericNode::new_prototype("add", "Add");

// A 输入：默认值为 0
add_node.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "A",
    PinTypeDesc::concrete(ValueType::Float64),
    Some(Value::from(0.0))  // 默认值
));

// B 输入：默认值为 0
add_node.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "B",
    PinTypeDesc::concrete(ValueType::Float64),
    Some(Value::from(0.0))
));
```

**前端显示**：
```
┌─────────────────┐
│      Add        │
├─────────────────┤
│ ○ A  [  0.0  ]  │  ← 输入框，可编辑
│ ○ B  [  0.0  ]  │  ← 输入框，可编辑
│         Sum  ○  │
└─────────────────┘
```

**用户操作**：
1. 用户修改 A 为 5.0 → `user_value = 5.0`
2. 用户连接常量到 B → 使用连接的值，输入框变灰
3. 执行：A = 5.0（用户值），B = 连接值

### 场景 2：字符串输入

```rust
// Print 节点
let print_node = GenericNode::new_prototype("print", "Print");

print_node.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "Message",
    PinTypeDesc::concrete(ValueType::String),
    Some(Value::from("Hello"))  // 默认消息
));
```

**前端显示**：
```
┌─────────────────────────┐
│        Print            │
├─────────────────────────┤
│ ○ Message [Hello     ]  │  ← 文本框
│                         │
└─────────────────────────┘
```

### 场景 3：布尔开关

```rust
// Branch 节点
let branch_node = GenericNode::new_prototype("branch", "Branch");

branch_node.add_in_data_pin(GenericInDataPin::new_with_default(
    uuid::Uuid::nil(),
    "Condition",
    PinTypeDesc::concrete(ValueType::Boolean),
    Some(Value::from(true))  // 默认为 true
));
```

**前端显示**：
```
┌─────────────────────────┐
│       Branch            │
├─────────────────────────┤
│ ▶ In                    │
│ ○ Condition  [✓]        │  ← 复选框
│              True  ▶    │
│              False ▶    │
└─────────────────────────┘
```

## 前端实现

### 1. Pin 组件

```typescript
// components/Pin.tsx

interface PinProps {
  pin: PinDto;
  onValueChange?: (value: any) => void;
}

function Pin({ pin, onValueChange }: PinProps) {
  const isConnected = pin.links.length > 0;
  const effectiveValue = pin.userValue ?? pin.defaultValue;
  
  return (
    <div className="pin">
      <div className="pin-connector" />
      <span className="pin-name">{pin.name}</span>
      
      {/* 只有输入 Pin 且未连接时显示控件 */}
      {pin.showWidget && !isConnected && (
        <PinWidget
          type={pin.type}
          widgetType={pin.widgetType}
          value={effectiveValue}
          onChange={onValueChange}
          disabled={isConnected}
        />
      )}
    </div>
  );
}
```

### 2. Pin 控件

```typescript
// components/PinWidget.tsx

interface PinWidgetProps {
  type: string;
  widgetType?: string;
  value: any;
  onChange: (value: any) => void;
  disabled: boolean;
}

function PinWidget({ type, widgetType, value, onChange, disabled }: PinWidgetProps) {
  // 根据类型选择控件
  switch (widgetType || type) {
    case 'float64':
    case 'int64':
      return (
        <input
          type="number"
          value={value ?? 0}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          disabled={disabled}
          className="pin-input-number"
        />
      );
    
    case 'string':
      return (
        <input
          type="text"
          value={value ?? ''}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          className="pin-input-text"
        />
      );
    
    case 'boolean':
      return (
        <input
          type="checkbox"
          checked={value ?? false}
          onChange={(e) => onChange(e.target.checked)}
          disabled={disabled}
          className="pin-input-checkbox"
        />
      );
    
    case 'color':
      return (
        <input
          type="color"
          value={value ?? '#000000'}
          onChange={(e) => onChange(e.target.value)}
          disabled={disabled}
          className="pin-input-color"
        />
      );
    
    case 'slider':
      return (
        <input
          type="range"
          min={0}
          max={100}
          value={value ?? 50}
          onChange={(e) => onChange(parseFloat(e.target.value))}
          disabled={disabled}
          className="pin-input-slider"
        />
      );
    
    default:
      return null;
  }
}
```

### 3. 更新 Pin 值

```typescript
// services/pinService.ts

export class PinService {
  /**
   * 更新 Pin 的用户值
   */
  static async updatePinValue(
    subgraphId: string,
    nodeId: string,
    pinId: string,
    value: any
  ): Promise<void> {
    await invoke('update_pin_user_value', {
      subgraphId,
      nodeId,
      pinId,
      value,
    });
  }
  
  /**
   * 清除 Pin 的用户值（恢复默认值）
   */
  static async clearPinValue(
    subgraphId: string,
    nodeId: string,
    pinId: string
  ): Promise<void> {
    await invoke('clear_pin_user_value', {
      subgraphId,
      nodeId,
      pinId,
    });
  }
}
```

## 后端实现

### 1. 更新 Pin 值命令

```rust
// src/commands/nodes.rs

#[tauri::command]
pub fn update_pin_user_value(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
    value: Value,
) -> Result<(), String> {
    let mut project = state.data.write().unwrap();
    let subgraph = get_subgraph_mut!(project, &subgraph_id)?;
    
    // 找到节点
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or("Node not found")?;
    
    // 找到 Pin 并更新用户值
    if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == pin_id) {
        pin.user_value = Some(value);
        return Ok(());
    }
    
    Err("Pin not found".to_string())
}

#[tauri::command]
pub fn clear_pin_user_value(
    state: State<'_, ProjectState>,
    subgraph_id: String,
    node_id: String,
    pin_id: String,
) -> Result<(), String> {
    let mut project = state.data.write().unwrap();
    let subgraph = get_subgraph_mut!(project, &subgraph_id)?;
    
    let node = subgraph.nodes.iter_mut()
        .find(|n| n.id == node_id)
        .ok_or("Node not found")?;
    
    if let Some(pin) = node.inputs.iter_mut().find(|p| p.id == pin_id) {
        pin.user_value = None;
        return Ok(());
    }
    
    Err("Pin not found".to_string())
}
```

### 2. 执行时获取值

```rust
// src/executor/context.rs

impl ExecutionContextTrait for ExecutionContext {
    fn get_pin_value(&mut self, pin_id: &str) -> Value {
        // 1. 查找运行时 Pin
        let runtime_pin_id = self.data_pin_id_to_runtime_pin_id
            .get(pin_id)
            .copied();
        
        if let Some(runtime_id) = runtime_pin_id {
            if let Some(node_id) = self.pin_to_node.get(&runtime_id) {
                if let Some(node) = self.nodes.get(node_id) {
                    let node_guard = node.lock().unwrap();
                    
                    // 2. 获取输入 Pin
                    if let Some(input_pin) = node_guard.get_input_concrete(&runtime_id) {
                        // 3. 使用 Pin 的有效值方法
                        return input_pin.get_effective_value(self);
                    }
                }
            }
        }
        
        Value::Null
    }
}
```

## 控件类型配置

### 节点定义时指定控件

```rust
// 创建带控件提示的 Pin
pub fn new_with_widget(
    id: PinId,
    name: &str,
    data_type: PinTypeDesc,
    default_value: Option<Value>,
    widget_type: Option<String>,
) -> Self {
    Self {
        id,
        name: name.to_string(),
        data_type,
        default_value,
        widget_type,
        user_value: RwLock::new(None),
        // ...
    }
}

// 示例：颜色选择器
let color_pin = GenericInDataPin::new_with_widget(
    uuid::Uuid::nil(),
    "Color",
    PinTypeDesc::concrete(ValueType::String),
    Some(Value::from("#FF0000")),
    Some("color".to_string())  // 使用颜色选择器
);

// 示例：滑块
let opacity_pin = GenericInDataPin::new_with_widget(
    uuid::Uuid::nil(),
    "Opacity",
    PinTypeDesc::concrete(ValueType::Float64),
    Some(Value::from(1.0)),
    Some("slider".to_string())  // 使用滑块
);
```

## 项目文件示例

```json
{
  "nodes": [
    {
      "id": "node_123",
      "type": "add",
      "title": "Add",
      "position": { "x": 100, "y": 100 },
      "inputs": [
        {
          "id": "pin_1",
          "name": "A",
          "type": "float64",
          "links": [],
          "defaultValue": 0.0,
          "userValue": 5.0,  // 用户设置为 5.0
          "showWidget": true
        },
        {
          "id": "pin_2",
          "name": "B",
          "type": "float64",
          "links": ["pin_3"],  // 已连接
          "defaultValue": 0.0,
          "userValue": null,   // 连接时忽略用户值
          "showWidget": true
        }
      ],
      "outputs": [
        {
          "id": "pin_4",
          "name": "Sum",
          "type": "float64",
          "links": []
        }
      ]
    }
  ]
}
```

## 最佳实践

### 1. 合理的默认值

```rust
// ✅ 好的默认值
add_pin.default_value = Some(Value::from(0.0));      // 数字：0
string_pin.default_value = Some(Value::from(""));    // 字符串：空
bool_pin.default_value = Some(Value::from(false));   // 布尔：false

// ❌ 避免 null 作为默认值（除非有特殊含义）
pin.default_value = None;  // 会导致类型不明确
```

### 2. 控件选择

```rust
// 数字范围：使用滑块
slider_pin.widget_type = Some("slider".to_string());

// 颜色：使用颜色选择器
color_pin.widget_type = Some("color".to_string());

// 长文本：使用文本域
text_pin.widget_type = Some("textarea".to_string());

// 枚举：使用下拉框
enum_pin.widget_type = Some("select".to_string());
```

### 3. 连接时的行为

```typescript
// 连接时禁用输入控件
<PinWidget
  disabled={pin.links.length > 0}
  value={pin.links.length > 0 ? null : effectiveValue}
/>

// 断开连接时恢复用户值或默认值
onDisconnect={() => {
  // 自动恢复到用户值或默认值
}}
```

## 总结

### 数据结构

| 字段 | 用途 | 来源 | 持久化 |
|------|------|------|--------|
| `defaultValue` | 节点定义的默认值 | 原型 | 否（可选） |
| `userValue` | 用户设置的值 | 用户输入 | 是 |
| `links` | 连接状态 | 连接操作 | 是 |

### 值优先级

```
连接值 > 用户值 > 默认值 > 零值
```

### 前端行为

- 未连接：显示输入控件，可编辑
- 已连接：隐藏或禁用控件，显示连接状态
- 断开连接：恢复用户值或默认值

### 后端实现

- `get_effective_value()`：按优先级获取值
- `update_pin_user_value()`：更新用户值
- `clear_pin_user_value()`：清除用户值
