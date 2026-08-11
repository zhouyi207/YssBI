# Remove Editor New Window Design

## Goal

Remove the Edit page window-menu action that opens another editor window and delete the code used only by that action.

## Scope

- Remove the **New Window** item and its adjacent leading separator from the Edit page window menu.
- Remove the `useMenubar` callback that invokes the action.
- Remove the dedicated `openSecondaryEditorWindow` module, its barrel exports, and its dedicated test file.
- Remove the unused `menubar.newWindow` English and Chinese translations.
- Do not change Rust because the action has no dedicated Rust command or backend workflow.

## Preserved behavior

- Keep **Open Logs in New Window**.
- Keep dragging the embedded log panel outside the main window to open a log window.
- Keep split-editor actions and all shared window creation, geometry persistence, and backend window-state infrastructure.
- Preserve unrelated working-tree changes, including `src-tauri/src/project/production_tests.rs`.

## Validation

The request explicitly excludes tests. Do not add or run tests. Run frontend type checking and `git diff --check` only to detect stale imports, type errors, and malformed patches.
