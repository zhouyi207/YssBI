export { EditorSessionProvider, useEditorSession } from './EditorSessionContext';
export type {
  EditorSession,
  EditorGroupSession,
  PickEditorSession,
  EditorSessionResourcesSlice,
  EditorSessionDetailActionsSlice,
  EditorSessionSyncCallbacksSlice,
} from './editorSessionTypes';
export {
  useEditorSessionResources,
  useEditorSessionDetailActions,
} from './useEditorSessionSlices';
export { useEditorOperations } from './useEditorOperations';
export { useEditorKeyboard } from './useEditorKeyboard';
export { useTabManagement } from './useTabManagement';
export {
  switchEditorTab,
  activateEditorGroup,
  activateCurrentEditorTab,
} from './switchEditorTab';
export {
  switchTab,
  closeTab,
  closeEditorGroup,
  splitEditorGroup,
  splitEditorGroupFromPointer,
  closeOtherTabs,
  closeAllTabsInGroup,
  closeSavedTabsInGroup,
  pinTab,
} from './tabCommands';
export { resolveTabDisplayName } from './resolveTabDisplayName';
export { reconcileOpenLayoutTabsWithResources } from './reconcileOpenLayoutTabs';
export { useProjectOperations } from './useProjectOperations';
export { useEditorGroup } from './useEditorGroup';
export { useIsActiveEditorGroup } from './useIsActiveEditorGroup';
export { CanvasContextMenuProvider, useCanvasContextMenuActions, useCanvasContextMenuActionsOptional } from './CanvasContextMenuContext';
export type { CanvasContextMenuActions } from './CanvasContextMenuContext';
export { useCanvasViewport } from './useCanvasViewport';
export { useCanvasWheelZoom } from './useCanvasWheelZoom';
export { useCanvasDrop } from './useCanvasDrop';
export type { VariableDropMenu } from './canvasDrop';
export { useCanvasOverlayHandlers } from './useCanvasOverlayHandlers';
export { saveAllDirtyGraphs } from './saveAllDirtyGraphs';