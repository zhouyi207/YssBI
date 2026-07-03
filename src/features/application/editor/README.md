# Editor Application Layer

Editor UI orchestration lives under `features/application/editor/`. Core editor state and actions live under `features/core/editor/`.

## Structure

```
features/application/editor/
├── useEditor.ts              # Composes core state + dataManagement hooks
├── useEditorGroup.ts         # Group-scoped wrapper (GroupContext from core)
├── useEditorOperations.ts    # Clipboard, history, node ops
├── useTabManagement.ts       # Tab open/close/switch
├── useProjectOperations.ts   # Save, load, execute
├── useCanvasViewport.ts      # Viewport persistence
├── useCanvasDrop.ts          # Canvas drop handlers
└── index.ts

features/core/editor/
├── hooks/                    # useEditorState, useEditorActions, …
├── context/GroupContext.ts   # Layout group scope for canvas
└── index.ts
```

## Usage

### Canvas / graph editing (with pointer loop)

```tsx
import { useEditorGroup } from '@/features/application/editor';

function CanvasArea() {
  const editor = useEditorGroup(); // withCanvasInteraction defaults true for editor groups
  // editor.onCanvasPointerDown, editor.onNodePointerDown, …
}
```

### Chrome only (Sidebar, Menubar, sync hooks)

```tsx
import { useEditor } from '@/features/application/editor';

const editor = useEditor({ withCanvasInteraction: false });
```

### Group scope

```tsx
import { GroupContext } from '@/features/core/editor';

<GroupContext.Provider value={groupId}>
  <GraphEditor />
</GroupContext.Provider>
```

`useEditorGroup()` reads `GroupContext` and scopes pointer handlers to that layout group.

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Resource actions: `features/application/resource/resourceActions.ts`
- Canvas interaction primitives: `features/core/canvas/`
