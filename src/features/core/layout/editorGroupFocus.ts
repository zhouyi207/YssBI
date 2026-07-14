import { listEditorGroupIds, firstEditorGroupId, isActiveEditorGroupValid } from './editorGridLayout';
import { readEditorPartOptions } from './editorPartOptions';
import { listEditorGroupTabIds } from './editorTabStore';
import { useLayoutStore } from './layoutStore';

/** VS Code MRU — next group to focus when closing the active empty group. */
export function getNextActiveEditorGroupId(excludeGroupId?: string): string | null {
  const state = useLayoutStore.getState();
  for (const groupId of state.recentEditorGroupIds) {
    if (groupId === excludeGroupId) continue;
    if (isActiveEditorGroupValid(state.nodes, groupId)) return groupId;
  }
  const fallback = firstEditorGroupId(state.nodes);
  if (fallback === excludeGroupId) {
    return listEditorGroupIds(state.nodes).find((id) => id !== excludeGroupId) ?? null;
  }
  return fallback;
}

/** Pre-activate MRU group before removing the last tab (VS Code `doCloseActiveEditor`). */
export function prepareActiveGroupBeforeLastTabClose(groupId: string): string | null {
  if (!readEditorPartOptions().closeEmptyGroups) return null;
  const state = useLayoutStore.getState();
  if (listEditorGroupIds(state.nodes).length <= 1) return null;
  if (listEditorGroupTabIds(groupId).length !== 1) return null;

  const nextGroupId = getNextActiveEditorGroupId(groupId);
  if (!nextGroupId) return null;

  useLayoutStore.getState().setActiveGroup(nextGroupId);
  return nextGroupId;
}
