# Editor Application Layer

Editor UI orchestration lives under `features/application/editor/`. Core editor state and actions live under `features/core/editor/`.

## Structure

```
features/application/editor/
├── EditorSessionContext.tsx  # Single EditorSessionProvider per Editor window
├── editorSessionTypes.ts     # 显式 EditorSession / EditorGroupSession 切片契约
├── useEditorSessionSlices.ts # useEditorSessionResources / DetailActions 等窄接口
├── useEditorSessionValue.ts  # 组装 session（无 pointer loop）
├── useEditorGroup.ts         # Group-scoped wrapper; optional canvas pointer loop
├── useEditorOperations.ts    # Clipboard, history, node ops
├── useTabManagement.ts       # Tab open/close/switch
└── index.ts

features/core/editor/
├── hooks/                    # useEditorState, useEditorActions, …
├── context/GroupContext.ts   # Layout group scope for canvas
└── index.ts
```

## Usage

### Editor window shell

```tsx
import { EditorSessionProvider } from '@/features/application/editor';

<EditorSessionProvider>
  <EditorWindowReady />
</EditorSessionProvider>
```

### Canvas (only place that mounts the pointer loop)

```tsx
const editor = useEditorGroup({ withCanvasInteraction: true });
```

### Chrome / overlays / sidebar (shared session, no pointer loop)

```tsx
const editor = useEditorGroup();
// or
const editor = useEditorSession();
```

### Project sync / auto-open hooks

```tsx
const editor = useEditorSession();
```

## API 约定

| Hook | 用途 | 挂载位置 |
|------|------|----------|
| `EditorSessionProvider` | 全窗口单例 session | `EditorWindow` 根节点 |
| `useEditorSession()` | 读共享 session（命令、tab、资源） | Provider 内任意 hook/组件 |
| `useEditorSessionResources()` | 仅 events/functions/variables/dataframes | Detail、侧栏资源列表 |
| `useEditorGroup()` | group 工作区 + 可选 canvas 交互 + 完整 session | Workspace / Canvas / Menubar |
| `useEditorGroup()` | 组级 scope + 可选 pointer 包装 | Sidebar、Menubar、Overlays 等 |
| `useEditorGroup({ withCanvasInteraction: true })` | 启用 canvas pointer loop | **仅** `Canvas.tsx` |

> `useEditor()` 已删除。Provider 外不应再构建独立 editor session。新 hook **禁止** `...session` 透传；使用 `editorSessionTypes` 中的 `PickEditorSession` / 切片 hook / `composeEditorGroupSession`。

设计约定详见 [DESIGN_RULE.md §2.12](../../../docs/DESIGN_RULE.md#212-editorsession-显式契约)。

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas interaction primitives: `features/core/canvas/`
