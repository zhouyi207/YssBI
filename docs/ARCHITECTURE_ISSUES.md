# 项目架构问题分析报告

> 基于侧边栏点击失效 Bug 排查过程中的深度分析

---

## 一、问题背景

在排查 **"有图时左键点击侧边栏项（events/functions/macros）无响应"** 这个 Bug 时，我们对项目的 Hook 链路、状态管理、事件系统进行了完整的追踪分析。最终定位到的根因是 **Zustand Store 无选择器调用** 与 **全局捕获阶段事件监听器** 的组合效应，导致 React 在 `pointerdown`（捕获阶段）和 `click`（冒泡阶段）之间触发了不必要的重渲染，使 React 事件委托丢失了 click 目标。

这次排查也暴露了项目中多个系统性的架构问题，以下逐一分析。

---

## 二、关键问题清单

### 问题 1：Zustand Store 无选择器调用（已部分修复）

**严重程度：🔴 高**

**问题描述：**

Zustand 的 `useStore()` 如果不传入选择器（selector），会返回整个 Store 对象。由于 Zustand 的 `set()` 每次调用都会创建新的状态引用（即使值没有变化），**任何** Store 字段的更新都会导致所有无选择器订阅者重渲染。

**受影响位置（共 6 处）：**

| 文件 | 代码 | 影响 |
|------|------|------|
| `useEditorOperations.ts` | `useClipboardStore()` | Canvas 组件链路全量重渲染 |
| `UIHost.tsx` | `useUIStore()` | 全局 UI 宿主不必要重渲染 |
| `SettingsView.tsx` | `useSettingsStore()` | 设置面板全量重渲染 |
| `LogWindow.tsx` | `useLogStore()` | 日志窗口全量重渲染 |
| `SelectionBox.tsx` | `useSelectionStore()` | 选区组件全量重渲染 |
| `useExecutionVisualization.ts` | `useExecutionStore()` | 执行可视化全量重渲染 |

**修复方式：**

```typescript
// ❌ 错误：订阅整个 Store，任何字段变化都会触发重渲染
const { clipboard, setClipboard } = useClipboardStore();

// ✅ 正确：仅订阅需要的字段，只有该字段变化才触发重渲染
const clipboard = useClipboardStore((s) => s.clipboard);
const setClipboard = useClipboardStore((s) => s.setClipboard);
```

> **原则：** 项目中所有 Zustand Store 调用都应使用选择器，永远不要解构整个 Store。

---

### 问题 2：全局事件监听器泛滥

**严重程度：🔴 高**

**问题描述：**

项目中有 **8 处** 使用 `window.addEventListener` / `document.addEventListener` 注册全局事件监听器。这些监听器在各自的 Hook 中独立管理，没有统一的协调机制，导致：

- 多个 `pointermove` 监听器同时活跃（Canvas 交互、Workspace 拖拽、键盘追踪）
- 捕获阶段监听器（`capture: true`）在所有组件的事件处理之前运行，容易产生副作用
- 清理时机依赖 React 的 Effect 生命周期，存在竞态风险

**清单：**

| 文件 | 事件 | 阶段 | 风险 |
|------|------|------|------|
| `useCanvasDrop.ts` | `pointerdown` | **捕获** | ⚠️ 每次点击都触发 Store 更新 |
| `useCanvasViewport.ts` | `wheel` | **捕获** | 阻止了全局滚轮默认行为 |
| `useCanvasInteraction.ts` | `pointermove` + `pointerup` | 冒泡 | ⚠️ 被多个组件实例化（见问题 3） |
| `useEditorKeyboard.ts` | `keydown` + `keyup` + `pointermove` | **捕获** | 全局键盘和指针追踪 |
| `Workspace.tsx` | `pointermove` | 冒泡 | 拖拽过程中追踪 |
| `Sash.tsx` | `mousemove` + `mouseup` | 冒泡 | 仅在 Sash 拖拽时 |
| `TabBar.tsx` | `mousemove` | 冒泡 | Tab 拖拽排序 |
| `Select.tsx` | `mousedown` | 冒泡 | 下拉菜单关闭 |

**修复建议：**

1. **建立全局事件管理器（EventBus/EventCoordinator）：** 统一注册和注销全局监听器，避免多实例重复注册。
2. **`handleClickOutside` 应限定作用范围：** 不应该在 `window` 的捕获阶段对每一次 `pointerdown` 都调用 Store setter。应检查点击是否在 Canvas 区域内，或在调用 `set()` 前先检查值是否已经为 `null`。
3. **监听器应添加 `AbortController` 支持：** 利用 `AbortSignal` 实现更可靠的清理。

---

### 问题 3：Hook 耦合度过高 —— `useEditorGroup` 链路过重

**严重程度：🔴 高**

**问题描述：**

`useEditorGroup()` 是项目中最核心的 Hook，但它的依赖链路极其深：

```
useEditorGroup()
  → useEditor()
      → useEditorState()          （订阅大量 Store）
      → useEditorActions()        （创建操作函数）
      → useEditorOperations()     （剪贴板、历史）
      → useTabManagement()        （标签页管理）
      → useProjectOperations()    （项目操作）
      → useGraphManagement()      （图管理）
      → useVariableManagement()   （变量管理）
      → useDatabaseManagement()   （数据库管理）
      → useNodeManagement()       （节点管理）
      → useCanvasInteraction()    （⚠️ 注册全局 pointermove/pointerup 监听器）
  → useEditorGroupWorkspace()
```

**问题在于：** 所有使用 `useEditorGroup()` 的组件都会实例化整条链路，包括 `useCanvasInteraction()`——即使该组件（如 Sidebar）根本不需要 Canvas 交互能力。

**受影响组件：**

| 组件 | 是否需要 Canvas 交互 | 是否被迫实例化 |
|------|---------------------|---------------|
| `Canvas.tsx` | ✅ 需要 | ✅ 合理 |
| `CanvasOverlays.tsx` | ✅ 需要 | ✅ 合理 |
| `Sidebar.tsx` | ❌ 不需要 | ⚠️ 被迫实例化 |
| `Menubar.tsx` | ❌ 不需要 | ⚠️ 被迫实例化 |
| `GraphEditor.tsx` | 部分需要 | ✅ 基本合理 |
| `LayoutNodeRenderer.tsx` | ❌ 不需要 | ⚠️ 被迫实例化 |

**后果：**

- Sidebar 等非 Canvas 组件也注册了全局 `pointermove`/`pointerup` 监听器
- 多个组件实例 → 多份重复的全局监听器
- 任何子 Hook 依赖变化都可能导致整条链路重新执行

**修复建议：**

```
方案 A（推荐）：拆分 Hook 层次

useEditorGroup()        ← 完整版，仅供 Canvas 组件使用
useEditorGroupLite()    ← 轻量版，供 Sidebar/Menubar 使用
                          仅包含：state、collections、tab management、
                          graph management、variable management
                          不包含：useCanvasInteraction

方案 B：将 useCanvasInteraction 改为单例模式

useCanvasInteraction 内部使用 ref 计数，
确保全局只注册一份 pointermove/pointerup 监听器，
无论多少组件实例化该 Hook。
```

---

### 问题 4：嵌套 DndContext（拖拽上下文冲突）

**严重程度：🟡 中**

**问题描述：**

项目中存在两个嵌套的 `DndContext`（来自 `@dnd-kit/core`）：

```
EditorWindow
  └─ DragProvider        ← DndContext #1（PointerSensor，无约束）
       └─ Workspace      ← DndContext #2（PointerSensor，delay: 150ms）
            └─ Sidebar（useDraggable 注册到 #2）
            └─ Canvas
```

- **DragProvider** 的 `DndContext` 没有注册任何 draggable 组件，其 `PointerSensor` 没有激活约束（立即激活）。
- **Workspace** 的 `DndContext` 是实际处理所有拖拽逻辑的上下文。
- `DragLayer`（`DragOverlay.tsx`）使用 `useDragContext()` 读取 DragProvider 的状态，但由于没有 draggable 注册到 DragProvider，`activeDrag` 永远为 `null`。

**后果：**

- DragProvider 的 `PointerSensor` 可能在文档级别添加不必要的事件监听器
- 两个 `DndContext` 的 Sensor 可能产生微妙的事件处理冲突
- `DragLayer` 组件实质上是死代码

**修复建议：**

移除 `DragProvider` 的 `DndContext`，仅保留 `DragContext.Provider`（用于共享拖拽状态）。或者直接删除 `DragProvider` 和 `DragLayer`，因为 `Workspace` 已经完全处理了拖拽逻辑。

---

### 问题 5：跨组件状态共享模式不一致

**严重程度：🟡 中**

**问题描述：**

项目中状态共享使用了多种模式，缺乏统一规范：

| 模式 | 使用场景 | 数量 |
|------|---------|------|
| Zustand Store（全局） | 编辑器状态、布局、图数据 | ~24 个 Store |
| React Context | GroupContext（编辑器组 ID） | 1 个 |
| React useState（局部） | 菜单状态、展开状态 | 多处 |
| Ref（命令式） | Canvas 位置、上次计数 | 多处 |
| 全局变量 | `canvasDropHandlerStore`、`(window as any)._lastAltKey` | 2-3 处 |

**问题点：**

1. **24 个 Zustand Store** 数量过多，部分 Store 的粒度过细（如 `useGestureStore` 仅管理一个 `gesture` 状态），部分过粗（如 `useEditorStore` 混合了 UI 状态和业务状态）。
2. **`(window as any)._lastAltKey`** 使用全局变量传递修饰键状态，绕过了 React 的数据流。
3. **`canvasDropHandlerStore`** 是一个手动管理的全局 Map，没有通过 React 的状态管理。

**修复建议：**

1. 合并相关 Store（例如 `useGestureStore` 和 `useSelectionStore` 可合并为 `useCanvasUIStore`）。
2. 用 Zustand Store 替代 `window` 全局变量。
3. 建立 Store 设计规范：按 **领域（domain）** 划分 Store，而非按功能碎片。

---

### 问题 6：`useMemo` 链路过深导致调试困难

**严重程度：🟡 中**

**问题描述：**

核心 Hook 链路中存在多层 `useMemo`：

```
useEditorGroup() → useMemo（合并 editor + workspace 数据）
  → useEditor() → useMemo（合并 state + actions + interactions）
      → useEditorState() → useMemo（合并 active + nodes + collections + groups + ui）
          → useEditorUIState()（无 useMemo，每次返回新对象）
          → useEditorCollections()（无 useMemo，每次返回新对象）
```

**问题点：**

1. `useEditorUIState()` 和 `useEditorCollections()` 没有使用 `useMemo`，每次调用返回新的对象引用。这导致上层 `useMemo` 的依赖总是变化，`useMemo` 形同虚设。
2. 多层 `useMemo` 带来的认知负担极高，排查"为什么组件重渲染了"需要逐层追踪。
3. 对象展开（`...state`、`...actions`）使得属性来源难以追踪。

**修复建议：**

1. 所有返回对象的子 Hook 都应使用 `useMemo`，确保引用稳定。
2. 考虑使用 `useShallow` （Zustand v4.5+ 提供）替代手动 `useMemo`。
3. 减少对象展开层数，使用明确的命名空间（如 `editor.canvas.xxx` 而非 `xxx`）。

---

## 三、架构优化路线图

### 第一阶段：紧急修复（1-2 天）

- [x] 修复 `useTabManagement` 和 `useDatabaseManagement` 的无选择器调用
- [x] 修复 `handleClickOutside` 跳过侧边栏事件
- [x] 修复剩余 6 处无选择器的 Store 调用
- [x] 移除 `DragProvider` 和 `DragLayer`（冗余嵌套 DndContext）

### 第二阶段：Hook 解耦（3-5 天）

- [x] Sidebar/detail 使用 `useCanvasInteraction({ enabled: false })`，避免重复全局监听
- [ ] 将 `useCanvasInteraction` 的全局监听器改为单例模式（可选，当前已通过 enabled 减少实例）
- [x] 为 `useEditorUIState` 和 `useEditorCollections` 添加 `useMemo`

### 第三阶段：状态管理重构（1-2 周）

- [ ] 合并过细的 Zustand Store（目标：24 → 12 个左右）
- [ ] 移除 `window` 全局变量，统一使用 Store
- [ ] 建立全局事件监听器管理器（EventCoordinator）
- [ ] 建立 Store 设计规范文档

---

## 四、总结

| 类别 | 问题数 | 影响范围 |
|------|--------|---------|
| 状态管理 | 6+ 处无选择器调用 | 全局性能 |
| 事件系统 | 8 处全局监听器无协调 | 事件冲突、点击失效 |
| Hook 耦合 | 6+ 组件被迫加载完整 Hook 链 | 内存、性能、可维护性 |
| DnD 架构 | 嵌套 DndContext + 死代码 | 潜在事件冲突 |
| 代码规范 | useMemo 不一致、全局变量 | 可维护性、调试难度 |

核心原则：**最小订阅、最小副作用、最小耦合**。每个组件应只订阅它需要的状态，只注册它需要的事件监听器，只依赖它需要的 Hook。
