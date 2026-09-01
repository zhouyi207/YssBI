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
├── useEditorCanvas.ts         # Panel-scoped Canvas commands/workspace/resources/interaction
├── useEditorOperations.ts     # Clipboard, history, node ops
├── useEditorPanelCommands.ts  # Narrow panel open/split facade
└── index.ts

features/core/editor/
├── hooks/                     # Group workspace and narrow collection hooks
├── context/GroupContext.ts    # Dockview group scope for layout-backed editors
└── index.ts
```

## Usage

### Editor window shell

```tsx
import { EditorSessionProvider } from "@/features/application/editor";

<EditorSessionProvider>
  <EditorWindowReady />
</EditorSessionProvider>;
```

### Canvas

`Canvas.tsx` is the sole caller of the panel-scoped hook:

```tsx
const canvas = useEditorCanvas({
  mode: "interactive",
  scope: { panelInstanceId, groupId, graphPath, graphKind },
});
```

Use `mode: 'preview'` for an inactive Dockview panel. Preview does not mount
the panel's context-menu actions, global pointer loop, or drop handling.

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

| Hook                                | Interface                                                                                            | Mount/caller                             |
| ----------------------------------- | ---------------------------------------------------------------------------------------------------- | ---------------------------------------- |
| `EditorSessionProvider`             | Stable command context                                                                               | `EditorWindow` root                      |
| `useEditorSessionCommandsContext()` | Explicit command-slice intersection                                                                  | Application consumers that need commands |
| `useEditorSessionResources()`       | Direct `useEditorCollections()` subscription for `events` / `functions` / `variables` / `dataframes` | Detail and resource lists                |
| `useEditorSessionDetailActions()`   | Variable/DataFrame update commands                                                                   | Detail panels                            |
| `useEditorCanvas({ mode, scope })`  | Panel-scoped Canvas `commands` / `workspace` / `resources` / `interaction`                           | **Only** `Canvas.tsx`                    |

Do not rebuild a broad editor/group aggregate, spread unrelated values through
a subtree, or mirror Dockview topology in a store. Add or reuse a named slice
at the caller seam instead.

Repository-wide dependency direction and Dockview authority rules are defined in
[`AGENTS.md`](../../../../AGENTS.md).

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas interaction primitives: `features/core/canvas/`
