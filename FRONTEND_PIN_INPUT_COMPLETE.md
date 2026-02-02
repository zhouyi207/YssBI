# 前端 Pin 输入功能实现完成

**日期**: 2026-02-02  
**状态**: ✅ 功能完成

## 📋 实现概述

为前端节点的未连接输入数据 Pin 添加了输入控件，允许用户直接在节点上设置值，并自动调用后端 API 保存。

## ✅ 已完成的工作

### 1. 新增组件

#### PinInput.tsx
```typescript
// 位置：src/components/Editor/Pins/PinInput.tsx
// 功能：根据 Pin 类型显示不同的输入控件
```

**支持的类型**：
- `int` - 整数输入框（步长 1）
- `float`/`number` - 浮点数输入框（步长 0.1）
- `bool`/`boolean` - 复选框
- `string` - 文本输入框
- 其他类型 - 通用文本输入框（支持 JSON）

**特性**：
- ✅ 自动保存到后端
- ✅ 键盘快捷键（Enter 保存，Escape 取消）
- ✅ 聚焦状态高亮
- ✅ 事件传播阻止（不干扰画布操作）

### 2. 更新的组件

#### Pin.tsx
```typescript
// 新增属性：
- subgraphId?: string;           // 子图 ID
- onValueChange?: (pinId, value) => void;  // 值变更回调

// 新增逻辑：
- 条件渲染输入控件
- 传递必要的上下文信息
```

**显示条件**：
```typescript
const showInput =
  direction === "input" &&    // 输入 Pin
  type !== "exec" &&          // 数据类型
  !isConnected &&             // 未连接
  subgraphId &&               // 有子图 ID
  nodeId;                     // 有节点 ID
```

#### Node.tsx
```typescript
// 新增属性：
- subgraphId?: string;
- onPinValueChange?: (pinId, value) => void;

// 更新：
- DefaultNodeUI 传递 subgraphId 和 onPinValueChange
- MathNodeUI 传递 subgraphId 和 onPinValueChange
```

#### Canvas.tsx
```typescript
// 更新节点渲染：
<Node
  {...props}
  subgraphId={activeTabId || undefined}
  onPinValueChange={(pinId, value) => {
    console.log(`Pin ${pinId} value changed:`, value);
  }}
/>
```

### 3. 后端 API 集成

```typescript
// 更新 Pin 值
await invoke("update_pin_user_value", {
  subgraphId: string,
  nodeId: string,
  pinId: string,
  value: any
});

// 清除 Pin 值
await invoke("clear_pin_user_value", {
  subgraphId: string,
  nodeId: string,
  pinId: string
});
```

### 4. 文档

- ✅ `src/components/Editor/Pins/README.md` - 详细使用文档

## 🎨 UI 效果

### 数学节点示例

```
┌─────────────────┐
│       Add       │
├─────────────────┤
│ ○ A    [  5  ]  │ ← 数字输入框
│ ○ B    [ 10  ]  │ ← 数字输入框
│         Out  ○  │
└─────────────────┘
```

### 字符串节点示例

```
┌───────────────────────┐
│    String Concat      │
├───────────────────────┤
│ ○ A    [ "Hello" ]    │ ← 文本输入框
│ ○ B    [ "World" ]    │ ← 文本输入框
│              Out  ○   │
└───────────────────────┘
```

### 布尔节点示例

```
┌─────────────────┐
│     Branch      │
├─────────────────┤
│ ▷ Exec          │
│ ○ Cond    [✓]   │ ← 复选框
│       True  ▷   │
│       False ▷   │
└─────────────────┘
```

## 🔄 数据流

```
用户输入
    ↓
PinInput 组件
    ↓
onChange 事件
    ↓
onBlur / 立即保存
    ↓
invoke("update_pin_user_value")
    ↓
后端保存到项目文件
    ↓
执行时使用三层优先级
(连接值 > 用户值 > 默认值)
```

## 📊 代码统计

### 新增文件
- `src/components/Editor/Pins/PinInput.tsx` - 200 行
- `src/components/Editor/Pins/README.md` - 文档

### 修改文件
- `src/components/Editor/Pins/Pin.tsx` - +30 行
- `src/components/Editor/Nodes/Node.tsx` - +20 行
- `src/components/Editor/Canvas/Canvas.tsx` - +5 行

### 总计
- **新增代码**: ~200 行
- **修改代码**: ~55 行
- **文档**: ~150 行

## 🎯 功能特性

### 1. 智能显示
- ✅ 仅在未连接的输入数据 Pin 上显示
- ✅ 连接后自动隐藏
- ✅ 断开连接后自动显示

### 2. 类型适配
- ✅ 根据 Pin 类型显示合适的控件
- ✅ 自动类型转换和验证
- ✅ 默认值支持

### 3. 用户体验
- ✅ 键盘快捷键支持
- ✅ 聚焦状态视觉反馈
- ✅ 紧凑的 UI 设计
- ✅ 不干扰画布操作

### 4. 数据持久化
- ✅ 自动保存到后端
- ✅ 项目文件持久化
- ✅ 加载时恢复值

## 🔧 技术实现

### 组件设计

```typescript
// PinInput 组件架构
PinInput
├── State Management
│   ├── value (本地状态)
│   └── isFocused (聚焦状态)
├── Event Handlers
│   ├── handleChange (值变更)
│   ├── handleBlur (失焦保存)
│   └── handleKeyDown (快捷键)
├── API Integration
│   └── invoke("update_pin_user_value")
└── Type-specific Renderers
    ├── NumberInput (int/float)
    ├── Checkbox (bool)
    ├── TextInput (string)
    └── GenericInput (其他)
```

### 样式系统

使用 Tailwind CSS 实现：
```css
/* 默认状态 */
bg-black/10 border-black/20

/* 聚焦状态 */
bg-black/20 border-blue-500 ring-1 ring-blue-500/50

/* 尺寸 */
w-16 h-5 text-[10px]  /* 数字 */
w-20 h-5 text-[10px]  /* 文本 */
w-4 h-4               /* 复选框 */
```

### 事件处理

```typescript
// 阻止事件传播
onClick={(e) => e.stopPropagation()}
onPointerDown={(e) => e.stopPropagation()}

// 键盘快捷键
onKeyDown={(e) => {
  if (e.key === "Enter") e.currentTarget.blur();
  if (e.key === "Escape") {
    setValue(initialValue);
    e.currentTarget.blur();
  }
}}
```

## 🚀 使用示例

### 基础用法

```tsx
// 在 Canvas 中渲染节点
<Node
  {...nodeProps}
  subgraphId={activeTabId}
  onPinValueChange={(pinId, value) => {
    console.log(`Pin ${pinId} changed to:`, value);
    // 可选：更新本地状态或触发其他操作
  }}
/>
```

### 自定义处理

```tsx
// 在 Canvas 组件中
const handlePinValueChange = useCallback((pinId: string, value: any) => {
  // 1. 记录日志
  console.log(`Pin ${pinId} value changed:`, value);
  
  // 2. 更新本地状态（可选）
  setNodes(prev => prev.map(node => {
    const pin = [...node.inputs, ...node.outputs].find(p => p.id === pinId);
    if (pin) {
      pin.defaultValue = value;
    }
    return node;
  }));
  
  // 3. 触发其他操作（可选）
  // 例如：实时预览、验证等
}, [setNodes]);
```

## 📝 注意事项

### 1. 连接优先级
- Pin 连接后，输入控件会自动隐藏
- 连接的值始终优先于用户设置的值

### 2. 类型安全
- 输入值会根据 Pin 类型进行转换
- 无效输入会被转换为默认值

### 3. 性能优化
- 使用 `useCallback` 避免不必要的重渲染
- 使用 `useState` 管理本地状态
- 失焦时才保存，减少 API 调用

### 4. 错误处理
- API 调用失败会在控制台输出错误
- 不会阻塞用户操作

## 🔮 未来改进

### 短期（1-2 周）
- [ ] 添加输入验证和错误提示
- [ ] 支持数组类型的输入
- [ ] 添加撤销/重做支持

### 中期（1-2 月）
- [ ] 滑块控件（用于数值范围）
- [ ] 颜色选择器（用于颜色类型）
- [ ] 下拉选择器（用于枚举类型）
- [ ] 自定义控件类型（通过 `widgetType` 字段）

### 长期（3+ 月）
- [ ] 表达式输入（支持简单计算）
- [ ] 资源选择器（文件、图片等）
- [ ] 实时预览（显示计算结果）
- [ ] 批量编辑（多选节点统一设置）

## 🎉 总结

成功实现了前端节点 Pin 输入功能，包括：

1. ✅ 完整的输入控件组件
2. ✅ 多种数据类型支持
3. ✅ 后端 API 集成
4. ✅ 良好的用户体验
5. ✅ 详细的文档

该功能为用户提供了直观的方式来设置节点参数，无需连接即可快速配置默认值，大大提升了工作效率。

与后端的动态 Pin 和用户值系统完美配合，形成了完整的节点参数管理解决方案。
