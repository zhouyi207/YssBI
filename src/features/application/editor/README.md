# Editor Application Layer

Editor UI orchestration lives under `features/application/editor/`. Core editor state and actions live under `features/core/editor/`.

## Structure

```
features/application/editor/
├── EditorSessionContext.tsx   # Stable command provider per Editor window
├── editorSessionCommands.ts   # Explicit intersection of stable command slices
├── editorSessionTypes.ts      # Named caller-shaped contracts
├── useEditorSessionCommands.ts
├── useEditorSessionSlices.ts  # Direct resource subscriptions and detail actions
├── useEditorCanvas.ts         # Canvas-only commands/workspace/resources/interaction
├── useEditorOperations.ts     # Clipboard, history, node ops
├── useTabManagement.ts        # Tab open/close/switch
└── index.ts

features/core/editor/
├── hooks/                     # Group workspace and narrow collection hooks
├── context/GroupContext.ts    # Dockview group scope for Canvas
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

### Canvas

`Canvas.tsx` is the sole caller of the caller-shaped hook:

```tsx
const canvas = useEditorCanvas({ mode: 'interactive' });
```

Use `mode: 'preview'` for an inactive Dockview group. Preview keeps the
pointerdown activation ordering but does not mount the global pointer loop,
drop handling, or `CanvasOverlays`.

`CanvasOverlays` receives a discriminated `graph` / `palette` / `variable` /
`execution` model from Canvas and must not call editor session hooks.

### Other consumers

Use the narrow interface matching the caller:

```tsx
const resources = useEditorSessionResources();
const detailActions = useEditorSessionDetailActions();
const commands = useEditorSessionCommandsContext();
```

## Interface rules

| Hook | Interface | Mount/caller |
|------|-----------|--------------|
| `EditorSessionProvider` | Stable command context | `EditorWindow` root |
| `useEditorSessionCommandsContext()` | Explicit command-slice intersection | Application consumers that need commands |
| `useEditorSessionResources()` | Direct `useEditorCollections()` subscription for `events` / `functions` / `variables` / `dataframes` | Detail and resource lists |
| `useEditorSessionDetailActions()` | Variable/DataFrame update commands | Detail panels |
| `useEditorCanvas({ mode })` | Canvas-only `commands` / `workspace` / `resources` / `interaction` | **Only** `Canvas.tsx` |

Do not rebuild a broad editor/group aggregate, spread unrelated values through
a subtree, or mirror Dockview topology in a store. Add or reuse a named slice
at the caller seam instead.

See [DESIGN_RULE.md §2.12](../../../../docs/architecture/DESIGN_RULE.md#212-editor-caller-shaped-显式契约).

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas interaction primitives: `features/core/canvas/`
