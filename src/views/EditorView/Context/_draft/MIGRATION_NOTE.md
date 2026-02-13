# Migration Note

## Date: 2026-02-13

## What Happened

The old `CanvasContext.tsx` and `CanvasProvider.tsx` have been refactored and moved to this `_draft` folder as backup.

### Old Files (Archived)
- `CanvasContext.old.tsx` - Original context definition and useCanvas hook
- `CanvasProvider.old.tsx` - Original provider with 500+ lines of mixed logic

### New Implementation

The logic has been refactored into a clean, modular structure in `src/features/editor/`:

```
src/features/editor/
├── stores/
│   ├── useEditorStore.ts      # Editor UI state
│   └── useClipboardStore.ts   # Clipboard state
├── hooks/
│   ├── useEditor.ts                  # Main hook (combines everything)
│   ├── useEditorOperations.ts       # Clipboard & history
│   ├── useTabManagement.ts          # Tab operations
│   ├── useProjectOperations.ts      # Project operations
│   ├── useSubGraphManagement.ts     # Events/Functions/Macros
│   ├── useVariableManagement.ts     # Variables
│   ├── useDataFrameManagement.ts    # DataFrames
│   └── useEditorKeyboard.ts         # Keyboard shortcuts
└── README.md                         # Complete documentation
```

### New Files (Active)
- `src/views/EditorView/Context/CanvasContext.tsx` - Simplified context (maintains same interface)
- `src/views/EditorView/Context/CanvasProvider.tsx` - New provider using hooks (60 lines)

## Why This Change?

1. **Separation of Concerns** - Each hook has a single responsibility
2. **Reusability** - Hooks can be used independently across components
3. **Testability** - Each piece can be tested in isolation
4. **Maintainability** - Much easier to understand and modify
5. **Performance** - Better optimization with granular hooks
6. **Type Safety** - Full TypeScript support throughout

## Backward Compatibility

The new implementation maintains the exact same interface as the old one, so all existing components continue to work without changes.

## If You Need to Rollback

1. Delete the new files:
   - `src/views/EditorView/Context/CanvasContext.tsx`
   - `src/views/EditorView/Context/CanvasProvider.tsx`

2. Restore the old files:
   ```bash
   copy _draft/CanvasContext.old.tsx ../CanvasContext.tsx
   copy _draft/CanvasProvider.old.tsx ../CanvasProvider.tsx
   ```

3. Delete the new feature folder (optional):
   ```bash
   rmdir /s src/features/editor
   ```

## Testing Checklist

- [x] Provider exports correctly
- [x] Context interface matches old version
- [x] useCanvas hook works the same way
- [ ] All canvas operations work (pan, zoom, select)
- [ ] Node operations work (create, delete, move)
- [ ] Pin connections work
- [ ] Copy/paste works
- [ ] Undo/redo works
- [ ] Save/load works
- [ ] Execute works
- [ ] Variables work
- [ ] Events/Functions/Macros work
- [ ] DataFrames work
- [ ] Keyboard shortcuts work
- [ ] Multi-group editing works

## Documentation

See `src/features/editor/README.md` for complete API documentation and usage examples.

## Questions?

If you encounter any issues with the new implementation, please:
1. Check the console for errors
2. Review the README.md in src/features/editor/
3. Compare behavior with the old implementation in this folder
4. Consider rolling back if critical issues are found
