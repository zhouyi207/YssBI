import type { EditorSession } from './editorSessionTypes';

/** Command surface merged into EditorSession / EditorGroupSession (excludes volatile history flags). */
export type EditorSessionCommands = Omit<
  EditorSession,
  | 'activeEditorGroupId'
  | 'activeTabId'
  | 'groupId'
  | 'tabs'
  | 'selectedNodeIds'
  | 'events'
  | 'functions'
  | 'variables'
  | 'dataframes'
  | 'groups'
  | 'contextMenu'
  | 'detailFocus'
  | 'pendingConnection'
  | 'canUndo'
  | 'canRedo'
  | 'pending'
>;

/** Stable identity container — Provider mutates fields in place so preview canvases avoid context churn. */
export function createEditorSessionCommandsContainer(): EditorSessionCommands {
  return {} as EditorSessionCommands;
}

export function patchEditorSessionCommands(
  target: EditorSessionCommands,
  patch: Partial<EditorSessionCommands>,
): EditorSessionCommands {
  Object.assign(target, patch);
  return target;
}
