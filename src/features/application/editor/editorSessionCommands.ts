import type {
  EditorSessionCanvasActions,
  EditorSessionDataframeActions,
  EditorSessionGraphActions,
  EditorSessionHistoryActions,
  EditorSessionLayoutBindings,
  EditorSessionNodeActions,
  EditorSessionProjectActions,
  EditorSessionTabActions,
  EditorSessionVariableActions,
  EditorSessionWorksheetActions,
} from './editorSessionTypes';

/** Stable command-only surface assembled by EditorSessionProvider. */
export type EditorSessionCommands = EditorSessionLayoutBindings
  & EditorSessionHistoryActions
  & EditorSessionCanvasActions
  & EditorSessionTabActions
  & EditorSessionWorksheetActions
  & EditorSessionProjectActions
  & EditorSessionGraphActions
  & EditorSessionVariableActions
  & EditorSessionDataframeActions
  & EditorSessionNodeActions;

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
