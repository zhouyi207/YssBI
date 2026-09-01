# Editor Application Layer

Editor UI orchestration lives under `features/application/editor/`. Core editor state and actions live under `features/core/editor/`.

## Structure

```
features/application/editor/
├── editorCanvasTypes.ts       # Panel-scoped Canvas contracts
├── workbenchCommandCapability.ts # Explicit Workbench command contract
├── useEditorCanvas.ts         # Panel-scoped Canvas commands/workspace/resources/interaction
├── useEditorOperations.ts     # Clipboard, history, node ops
├── useEditorPanelCommands.ts  # Narrow panel open/split facade
├── useDetailsCommands.ts      # Details-only mutation capability
└── index.ts

features/core/editor/
├── hooks/                     # Narrow projection and UI hooks
└── index.ts
```

## Usage

### Workbench composition

```tsx
const commands = useWorkbenchCommandCoordinator();

<WorkbenchWindow commands={commands} />;
```

Only `app/integrations/workbenchCommandCoordinator.ts` combines menu, keyboard,
project, graph, and chart commands. Views receive the typed capability through props.

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
`execution` model from Canvas and must not assemble application commands.

### Other consumers

Use the narrow capability matching the caller:

```tsx
const { updateVariable } = useDetailsCommands();
const activeGraph = useActiveProjectGraph();
```

## Interface rules

| Capability / Hook                  | Interface                                                                  | Mount/caller                |
| ---------------------------------- | -------------------------------------------------------------------------- | --------------------------- |
| `WorkbenchCommandCapability`       | Menu, keyboard, and welcome actions composed by the app                    | `WorkbenchWindow` props     |
| `useDetailsCommands()`             | Variable mutation required by Details                                      | `DetailsPane`               |
| `useActiveProjectGraph()`          | Active event/function resource required by Project Explorer                | Project Activity controller |
| `useEditorCanvas({ mode, scope })` | Panel-scoped Canvas `commands` / `workspace` / `resources` / `interaction` | **Only** `Canvas.tsx`       |

Do not rebuild a broad editor/group aggregate, spread unrelated values through
a subtree, or mirror Dockview topology in a store. Add or reuse a named slice
at the caller seam instead.

Repository-wide dependency direction and Dockview authority rules are defined in
[`AGENTS.md`](../../../../AGENTS.md).

## Related modules

- Graph CRUD UI: `features/application/dataManagement/useGraphManagement.ts`
- Canvas interaction primitives: `features/core/canvas/`
