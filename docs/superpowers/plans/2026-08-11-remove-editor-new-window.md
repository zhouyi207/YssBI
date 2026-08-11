# Remove Editor New Window Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Remove the Edit page window-menu action that creates another editor window and delete code used only by that action.

**Architecture:** Delete the complete dedicated frontend call chain from the menu through the application window helper. Preserve shared auxiliary-window infrastructure because logs, database tools, Bayes, and presentation windows still depend on it; no Rust code changes because no dedicated Rust command exists for this action.

**Tech Stack:** React, TypeScript, Tauri, i18next, pnpm

## Global Constraints

- Keep **Open Logs in New Window** and log-panel drag detachment unchanged.
- Keep split-editor actions and shared window creation, geometry persistence, and Rust window-state infrastructure unchanged.
- Do not modify the unrelated working-tree change in `src-tauri/src/project/production_tests.rs`.
- Do not add or run tests, per the explicit request.
- Do not create a commit unless explicitly requested.

---

### Task 1: Delete the dedicated editor-new-window call chain

**Files:**
- Modify: `src/views/EditorView/Layout/Menubar.tsx:133-149,215-222`
- Modify: `src/features/application/menubar/useMenubar.ts:16-24,140-164`
- Modify: `src/features/application/window/index.ts:31-34`
- Modify: `src/app/i18n/locales/en-US.ts:190-194`
- Modify: `src/app/i18n/locales/zh-CN.ts:190-194`
- Delete: `src/features/application/window/openSecondaryEditorWindow.ts`
- Delete: `src/features/application/window/openSecondaryEditorWindow.test.ts`

**Interfaces:**
- Removes: `openSecondaryEditorWindow(): Promise<void>`
- Removes: `buildSecondaryEditorWindowRequest(label: string): PersistedWindowOptions`
- Preserves: `openLogsWindow(options?: OpenLogsWindowOptions): Promise<void>`

- [ ] **Step 1: Remove the menu entry and its leading separator**

In `src/views/EditorView/Layout/Menubar.tsx`, remove `openNewWindow` from the `useMenubar()` destructuring and change the window menu to:

```tsx
const windowItems: MenuItem[] = [
  { label: t("menubar.splitEditorRight"), onClick: handleSplitRight },
  { label: t("menubar.splitEditorDown"), onClick: handleSplitDown },
  { label: "-" },
  { label: t("menubar.openLogsInNewWindow"), onClick: handleOpenLogs },
];
```

- [ ] **Step 2: Remove the menubar application callback**

In `src/features/application/menubar/useMenubar.ts`, remove `openSecondaryEditorWindow` from the window import, delete the `openNewWindow` callback, and remove `openNewWindow` from the returned object. Keep `logger` because the close listener still uses it.

The retained window import must be:

```ts
import {
  openBayesWindow,
  openDatabaseEditorWindow,
  openLogsWindow,
} from "@/features/application/window";
```

- [ ] **Step 3: Remove the dedicated module and exports**

Delete:

```text
src/features/application/window/openSecondaryEditorWindow.ts
src/features/application/window/openSecondaryEditorWindow.test.ts
```

Remove this export block from `src/features/application/window/index.ts`:

```ts
export {
  buildSecondaryEditorWindowRequest,
  openSecondaryEditorWindow,
} from "./openSecondaryEditorWindow";
```

Do not remove `createPersistedWindow`, `createEphemeralWindowLabel`, or secondary-window geometry helpers because other auxiliary-window workflows and editor window persistence still use them.

- [ ] **Step 4: Remove unused translations**

Remove only these properties and retain neighboring menu strings:

```ts
newWindow: "New Window",
```

```ts
newWindow: "新窗口",
```

- [ ] **Step 5: Check for stale references**

Search `src/` for:

```text
openSecondaryEditorWindow
buildSecondaryEditorWindowRequest
menubar.newWindow
openNewWindow
```

Expected: no matches related to the removed editor-new-window action.

- [ ] **Step 6: Run non-test validation**

Run:

```bash
pnpm typecheck
git diff --check
```

Expected: both commands exit with status 0. Do not run Vitest or Rust tests.

- [ ] **Step 7: Review scope preservation**

Inspect the final diff and confirm:

- `src-tauri/src/project/production_tests.rs` remains untouched by this task.
- `openLogsWindow` and `useLogPanelDetach` are unchanged.
- No Rust file is changed by this task.
- The design and plan documents are the only documentation additions.
