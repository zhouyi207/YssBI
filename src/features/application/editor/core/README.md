# Editor Feature

This feature contains all the editor-related logic that was previously in `CanvasContext` and `CanvasProvider`.

**Note:** As of the final migration, `CanvasProvider` and `CanvasContext` have been completely removed. All components now use hooks directly without requiring a Context Provider.

## Structure

```
editor/
├── stores/
│   ├── useEditorStore.ts      # Editor UI state (context menu, selection, pending connection)
│   ├── useClipboardStore.ts   # Clipboard operations state
│   └── index.ts
├── hooks/
│   ├── useEditor.ts                  # Main hook combining all functionality
│   ├── useEditorOperations.ts       # Clipboard, history, node operations
│   ├── useTabManagement.ts          # Tab opening, closing, switching
│   ├── useProjectOperations.ts      # Save, load, execute operations
│   ├── useSubGraphManagement.ts     # Events, functions, macros management
│   ├── useVariableManagement.ts     # Variable CRUD and promotion/demotion
│   ├── useDataFrameManagement.ts    # DataFrame CRUD
│   ├── useEditorKeyboard.ts         # Keyboard shortcuts
│   ├── useEditorGroup.ts            # Context-aware editor hook
│   └── index.ts
└── index.ts
```

## Usage

### Recommended: Use useEditorGroup for Context-Aware Components

For components that need context-aware editor operations (like components within a GroupContext):

```tsx
import { useEditorGroup, GroupContext } from '@/features/editor';

function MyComponent() {
  const editor = useEditorGroup();
  
  // Automatically scopes to the current group context
  // Provides groupId, tabs, activeTabId, nodes, variables, etc.
  editor.addEvent();
  editor.saveGraph();
  console.log(editor.groupId); // Current group ID
}

// Use with GroupContext for scoped operations
function ParentComponent() {
  return (
    <GroupContext.Provider value="my-group-id">
      <MyComponent />
    </GroupContext.Provider>
  );
}
```

### Use useEditor for Global Operations

For components that work with the globally active editor:

```tsx
import { useEditor } from '@/features/editor';

function MyComponent() {
  const editor = useEditor();
  
  // Access any editor functionality
  editor.addEvent();
  editor.saveGraph();
  editor.copy();
  // etc.
}
```

### No Provider Needed!

Unlike the old architecture, you don't need to wrap your app in a Provider:

```tsx
// ❌ Old way (removed)
<CanvasProvider>
  <MyComponent />
</CanvasProvider>

// ✅ New way (no provider needed)
<MyComponent />
```

### Granular Usage

Import specific hooks for more control:

```tsx
import { useEditorOperations, useTabManagement } from '@/features/editor';

function MyComponent() {
  const { copy, paste, undo, redo } = useEditorOperations();
  const { openSubGraph, closeTab } = useTabManagement();
}
```

## Migration from CanvasProvider

The old `CanvasProvider` has been refactored into:

1. **Stores** - State management using Zustand
   - `useEditorStore` - UI state (context menu, selection, etc.)
   - `useClipboardStore` - Clipboard state

2. **Hooks** - Business logic and operations
   - `useEditor` - Main hook combining everything
   - `useEditorOperations` - Clipboard and history operations
   - `useTabManagement` - Tab operations
   - `useProjectOperations` - Project-level operations
   - `useSubGraphManagement` - SubGraph CRUD
   - `useVariableManagement` - Variable operations
   - `useDataFrameManagement` - DataFrame operations
   - `useEditorKeyboard` - Keyboard shortcuts

3. **Simplified Provider** - `CanvasProvider.new.tsx`
   - Uses the new hooks
   - Much simpler and cleaner
   - Easy to understand and maintain

## Benefits

1. **Separation of Concerns** - Each hook has a single responsibility
2. **Reusability** - Hooks can be used independently
3. **Testability** - Each hook can be tested in isolation
4. **Type Safety** - Full TypeScript support
5. **Performance** - Better optimization with granular hooks
6. **Maintainability** - Easier to understand and modify

## API Reference

### useEditor()

Main hook that combines all editor functionality.

**Returns:**
- All state and operations from sub-hooks
- Canvas interaction handlers
- Project operations
- SubGraph management
- Variable management
- DataFrame management
- Clipboard operations
- History operations

### useEditorOperations()

Handles clipboard operations and history.

**Returns:**
- `copy()` - Copy selected nodes
- `cut()` - Cut selected nodes
- `paste(pos?)` - Paste nodes at position
- `deleteSelected()` - Delete selected nodes
- `undo()` - Undo last operation
- `redo()` - Redo last undone operation
- `saveHistory()` - Save current state to history
- `canUndo` - Whether undo is available
- `canRedo` - Whether redo is available

### useTabManagement()

Handles tab operations.

**Returns:**
- `setActiveTabId(id, targetGroupId?)` - Set active tab
- `openSubGraph(id, name, type, data?)` - Open a subgraph tab
- `openSettingsTab()` - Open settings tab
- `closeTab(id, e?)` - Close a tab
- `splitEditorRight(groupId)` - Split editor to the right
- `closeGroup(id)` - Close an editor group

### useProjectOperations(openSubGraph)

Handles project-level operations.

**Parameters:**
- `openSubGraph` - Function to open a subgraph

**Returns:**
- `saveGraph()` - Save current project
- `saveGraphAs()` - Save project as new file
- `importGraph(json?)` - Import project from file or JSON
- `executeGraph()` - Execute current event
- `executeAllEvents()` - Execute all events

### useSubGraphManagement(openSubGraph, closeTab, switchSidebarTab)

Handles events, functions, and macros.

**Parameters:**
- `openSubGraph` - Function to open a subgraph
- `closeTab` - Function to close a tab
- `switchSidebarTab` - Function to switch sidebar tab

**Returns:**
- `addEvent(name?)` - Create new event
- `updateEvent(id, data)` - Update event
- `deleteEvent(id)` - Delete event
- `addFunction(name?)` - Create new function
- `updateFunction(id, data)` - Update function
- `deleteFunction(id)` - Delete function
- `addMacro(name?)` - Create new macro
- `updateMacro(id, data)` - Update macro
- `deleteMacro(id)` - Delete macro

### useVariableManagement(switchSidebarTab)

Handles variable operations.

**Parameters:**
- `switchSidebarTab` - Function to switch sidebar tab

**Returns:**
- `addVariable(name?, type?, isGlobal?)` - Create variable
- `updateVariable(id, data)` - Update variable
- `deleteVariable(id)` - Delete variable
- `promoteVariable(id)` - Promote to global variable
- `demoteVariable(id)` - Demote to local variable

### useDataFrameManagement()

Handles DataFrame operations.

**Returns:**
- `addDataFrame(name?)` - Create DataFrame
- `updateDataFrame(id, data)` - Update DataFrame
- `deleteDataFrame(id)` - Delete DataFrame

### useEditorKeyboard(props)

Handles keyboard shortcuts.

**Parameters:**
- Object with all operation functions

**Side Effects:**
- Sets up global keyboard event listeners
- Cleans up on unmount
