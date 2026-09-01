import { focusEditorGroupSync, hydrateEditorGroup } from "./switchEditorTab";

export const EDITOR_GROUP_TAB_CHROME_SELECTOR =
  "[data-tab-id], [data-tab-strip], [data-tabbar-drop], [data-editor-group-actions]";

export function shouldSkipEditorGroupShellActivation(target: EventTarget | null): boolean {
  return target instanceof HTMLElement && !!target.closest(EDITOR_GROUP_TAB_CHROME_SELECTOR);
}

/** VS Code editor MOUSE_DOWN — sync layout focus; graph hydrate continues async. */
export function prepareEditorGroupForInteraction(groupId: string): void {
  if (focusEditorGroupSync(groupId)) void hydrateEditorGroup(groupId);
}
