# 动态 Pin 完整流程

## 核心理解

### 节点的两种形态

```
┌─────────────────────────────────────────────────────────┐
│                    注册表原型                              │
│  ┌──────────────────────────────────────────────┐       │
│  │  dynamic_add (Prototype)                     │       │
│  │  - Input 1 (默认)                             │       │
│  │  - Input 2 (默认)                             │       │
│  │  - Sum (输出)                                 │       │
│  │  - DynamicCapability (配置)                   │       │
│  └──────────────────────────────────────────────┘       │
│         ↓ 克隆                                           │
└─────────────────────────────────────────────────────────┘
          ↓
┌─────────────────────────────────────────────────────────┐
│                   项目中的实例                             │
│  ┌──────────────────────────────────────────────┐       │
│  │  Node #123 (Instance)                        │       │
│  │  - Input 1                                   │       │
│  │  - Input 2                                   │       │
│  │  - Input 3 (动态添加) ← dynamicPins 元数据     │       │
│  │  - Sum (输出)                                 │       │
│  └──────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────┘
```

## 关键点

### 1. 创建新节点 = 使用原型

```
用户点击 "Add Node" → dynamic_add
    ↓
从注册表克隆原型
    ↓
新节点有 2 个输入（默认）
    ↓
dynamicPins = undefined
```

### 2. 添加 Pin = 修改实例

```
用户点击 "+ Add Input"
    ↓
调用 add_node_dynamic_pin()
    ↓
节点现在有 3 个输入
    ↓
dynamicPins = [{ pinId: "...", name: "Input 3", ... }]
    ↓
保存到项目文件
```

### 3. 复制节点 = 复制实例状态

```
用户复制节点（3 个输入）
    ↓
克隆 SerializedNode（包括 dynamicPins）
    ↓
新节点也有 3 个输入
    ↓
dynamicPins 也被复制
```

### 4. 加载项目 = 恢复实例状态

```
打开项目文件
    ↓
读取 SerializedNode
    ↓
从注册表获取原型（2 个输入）
    ↓
应用 dynamicPins 元数据
    ↓
恢复节点（3 个输入）
```

## 数据结构对比

### 原型（注册表）

```rust
// 只定义默认状态
GenericNode {
    node_type: "dynamic_add",
    inputs: [
        Pin { name: "Input 1" },
        Pin { name: "Input 2" },
    ],
    outputs: [
        Pin { name: "Sum" },
    ],
    dynamic_capability: Some(...),  // 支持动态 Pin
}
```

### 实例（项目文件）

```json
{
  "id": "node_123",
  "type": "dynamic_add",
  "inputs": [
    { "id": "pin_1", "name": "Input 1" },
    { "id": "pin_2", "name": "Input 2" },
    { "id": "pin_3", "name": "Input 3" }  // 动态添加的
  ],
  "outputs": [
    { "id": "pin_4", "name": "Sum" }
  ],
  "dynamicPins": [  // 🔑 关键：记录哪些是动态添加的
    {
      "pinId": "pin_3",
      "pinType": "Data",
      "direction": "Input",
      "name": "Input 3",
      "isDynamic": true
    }
  ]
}
```

## 为什么需要 dynamicPins 字段？

### 问题：如何区分默认 Pin 和动态 Pin？

```
场景 1：加载项目
- 节点有 3 个输入
- 问题：哪些是默认的？哪些是动态添加的？
- 解决：检查 dynamicPins 字段

场景 2：复制节点
- 源节点有 3 个输入
- 问题：新节点应该有几个输入？
- 解决：复制 dynamicPins，保持一致

场景 3：移除 Pin
- 用户想移除 Input 3
- 问题：是否允许移除？
- 解决：检查 dynamicPins，只能移除动态添加的
```

## 实现要点

### 1. SerializedNode 扩展

```rust
pub struct SerializedNode {
    // 现有字段...
    pub inputs: Vec<SerializedPin>,
    pub outputs: Vec<SerializedPin>,
    
    // 🆕 新增字段
    #[serde(rename = "dynamicPins", skip_serializing_if = "Option::is_none")]
    pub dynamic_pins: Option<Vec<DynamicPinMetadata>>,
}
```

### 2. 添加 Pin 时更新元数据

```rust
// 添加 Pin 到 inputs
node.inputs.push(new_pin);

// 🔑 同时记录元数据
if node.dynamic_pins.is_none() {
    node.dynamic_pins = Some(vec![]);
}
node.dynamic_pins.as_mut().unwrap().push(metadata);
```

### 3. 加载时恢复动态 Pin

```rust
// 从原型克隆
let node = prototype.clone();

// 🔑 恢复动态 Pin
if let Some(dynamic_pins) = &serialized_node.dynamic_pins {
    node.rebuild_from_dynamic_info(dynamic_pins)?;
}
```

## 用户体验

### ✅ 正确的行为

```
1. 创建 dynamic_add 节点
   → 2 个输入（默认）

2. 添加输入
   → 3 个输入

3. 保存项目
   → dynamicPins 保存到文件

4. 关闭并重新打开
   → 3 个输入（恢复）

5. 复制节点
   → 新节点也是 3 个输入

6. 再创建一个新的 dynamic_add
   → 2 个输入（默认）
```

### ❌ 错误的行为（如果不持久化）

```
1. 创建 dynamic_add 节点
   → 2 个输入

2. 添加输入
   → 3 个输入

3. 保存项目
   → ❌ dynamicPins 没有保存

4. 关闭并重新打开
   → ❌ 只有 2 个输入（丢失了动态 Pin）

5. 复制节点
   → ❌ 新节点只有 2 个输入（应该是 3 个）
```

## 总结

| 操作 | 数据来源 | Pin 数量 |
|------|---------|---------|
| 创建新节点 | 注册表原型 | 默认（2 个） |
| 添加 Pin | 修改实例 + 保存元数据 | 增加（3 个） |
| 复制节点 | 克隆实例 + 复制元数据 | 保持（3 个） |
| 加载项目 | 原型 + 应用元数据 | 恢复（3 个） |

**核心**：`dynamicPins` 字段是动态 Pin 持久化的关键，它记录了哪些 Pin 是动态添加的，以及如何恢复它们。
