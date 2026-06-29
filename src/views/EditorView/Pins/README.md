# Pin 输入功能

## 概述

为未连接的输入数据 Pin 添加了输入控件，允许用户直接在节点上设置 Pin 的值。

## 功能特性

### 1. 自动显示输入控件

当满足以下条件时，Pin 会自动显示输入控件：
- Pin 是输入方向（`direction === "input"`）
- Pin 是数据类型（`type !== "exec"`）
- Pin 未连接（`connected === false` 且 `linkCount === 0`）
- 有有效的 `subgraphId` 和 `nodeId`

### 2. 支持的数据类型

根据 Pin 类型显示不同的输入控件：

| Pin 类型 | 控件类型 | 说明 |
|---------|---------|------|
| `int` | 数字输入框 | 整数，步长为 1 |
| `float`, `number` | 数字输入框 | 浮点数，步长为 0.1 |
| `bool`, `boolean` | 复选框 | 布尔值 |
| `string` | 文本输入框 | 字符串 |
| 其他 | 通用文本输入框 | 尝试 JSON 解析 |

### 3. 值的优先级

Pin 值遵循三层优先级（后端实现）：
1. **连接值**（最高）- 如果 Pin 有连接，使用连接的值
2. **用户值**（中等）- 如果没有连接但用户设置了值，使用用户值
3. **默认值**（最低）- 如果都没有，使用默认值

## 使用方法

### 前端使用

```tsx
import { Pin } from "./Pins/Pin";

<Pin
  {...pinData}
  subgraphId="event-1"  // 必需：子图 ID
  nodeId="node-123"     // 必需：节点 ID（通过 Pin 的 nodeId 属性）
  onValueChange={(pinId, value) => {
    console.log(`Pin ${pinId} changed to:`, value);
  }}
/>
```

### 后端 API

输入控件会自动调用后端 API 保存值：

```typescript
// 更新 Pin 值
await invoke("update_pin_user_value", {
  subgraphId: "event-1",
  nodeId: "node-123",
  pinId: "pin-456",
  value: 42
});

// 清除 Pin 值（恢复默认值）
await invoke("clear_pin_user_value", {
  subgraphId: "event-1",
  nodeId: "node-123",
  pinId: "pin-456"
});
```

## 组件结构

```
Pin.tsx
├── Pin 图标和标签
└── PinInput.tsx (条件渲染)
    ├── 数字输入框 (int/float)
    ├── 复选框 (bool)
    ├── 文本输入框 (string)
    └── 通用输入框 (其他类型)
```

## 交互行为

### 键盘快捷键

- **Enter** - 保存并失焦
- **Escape** - 取消编辑并恢复原值

### 自动保存

- 数字和文本输入：失焦时保存
- 布尔值：立即保存

### 事件阻止

输入控件会阻止以下事件传播，避免干扰画布操作：
- `onClick`
- `onPointerDown`

## 样式

输入控件使用 Tailwind CSS 样式：
- 默认：半透明黑色背景
- 聚焦：蓝色边框和环形高亮
- 尺寸：紧凑型（适合节点内部）

## 示例

### 数学节点

```
┌─────────────┐
│     Add     │
├─────────────┤
│ A  [  5  ]  │ ← 输入框
│ B  [ 10  ]  │ ← 输入框
│         Out │
└─────────────┘
```

### 字符串节点

```
┌─────────────────┐
│  String Concat  │
├─────────────────┤
│ A  [ "Hello" ]  │ ← 文本输入框
│ B  [ "World" ]  │ ← 文本输入框
│           Out   │
└─────────────────┘
```

### 布尔节点

```
┌─────────────┐
│   Branch    │
├─────────────┤
│ Exec        │
│ Cond  [✓]   │ ← 复选框
│       True  │
│       False │
└─────────────┘
```

## 注意事项

1. **连接优先**：一旦 Pin 被连接，输入控件会自动隐藏
2. **类型安全**：输入值会根据 Pin 类型进行验证和转换
3. **性能优化**：使用 `useCallback` 和 `useState` 优化渲染
4. **错误处理**：API 调用失败会在控制台输出错误

## 未来改进

- [ ] 添加滑块控件（用于数值范围）
- [ ] 添加颜色选择器（用于颜色类型）
- [ ] 添加下拉选择器（用于枚举类型）
- [ ] 支持数组类型的输入
- [ ] 添加输入验证和错误提示
- [ ] 支持自定义控件类型（通过 `widgetType` 字段）
