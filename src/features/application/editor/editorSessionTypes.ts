/**
 * EditorSession 显式契约 — 按职责切片，避免从组合 hook 建立推断链。
 * 新 hook 应依赖命名切片，禁止 `...session` 透传未知字段。
 */

import type { RefObject } from 'react';
import type { buildEditorState } from '@/features/core/editor/hooks/useEditorState';
import type { useEditorActions } from '@/features/core/editor/hooks/useEditorActions';
import type { EditorViewport } from '@/features/core/viewport';
import type { GraphSelection } from '@/features/core/layout';
import type { CanvasMutationOutcome } from '@/features/core/canvas';
import type { Pin } from '@/shared/types/domain/pin';
import type { EditorCommandTarget } from './editorCommandFocus';
import type { useEditorOperations } from './useEditorOperations';
import type { useTabManagement } from './useTabManagement';
import type { useOpenWorksheet, useWorksheetManagement } from './useWorksheetManagement';
import type { useProjectOperations } from './useProjectOperations';
import type { useGraphManagement } from '@/features/application/dataManagement/useGraphManagement';
import type { useVariableManagement } from '@/features/application/dataManagement/useVariableManagement';
import type { useDatabaseManagement } from '@/features/application/dataManagement/useDatabaseManagement';
import type { useNodeManagement } from '@/features/application/dataManagement/useNodeManagement';

// ─── State & layout bindings ───────────────────────────────────────────────

export type EditorSessionState = ReturnType<typeof buildEditorState>;

type EditorActions = ReturnType<typeof useEditorActions>;

/** Provider 暴露的布局 / UI / canvas ref 绑定（非完整 useEditorActions） */
export interface EditorSessionLayoutBindings {
  setCanvas: EditorActions['setCanvas'];
  setContextMenu: EditorActions['setContextMenu'];
  setDetailFocus: EditorActions['setDetailFocus'];
  clearDetailFocus: EditorActions['clearDetailFocus'];
  activeGroupIdRef: RefObject<string | null>;
  activeTabIdRef: RefObject<string | null>;
  viewportRef: RefObject<EditorViewport>;
}

// ─── Command slices（与各 application hook 1:1）────────────────────────────

export type EditorSessionHistoryActions = ReturnType<typeof useEditorOperations>;
export interface EditorSessionCanvasActions {
  selectAllNodes(target: EditorCommandTarget): Promise<boolean>;
  focusSelectedNodes(target: EditorCommandTarget): boolean;
  fitCompleteGraph(target: EditorCommandTarget): boolean;
}

export type EditorSessionHistoryAvailability = {
  canUndo: boolean;
  canRedo: boolean;
  pending: boolean;
};
export type EditorSessionTabActions = ReturnType<typeof useTabManagement>;
export type EditorSessionWorksheetActions = ReturnType<typeof useWorksheetManagement> & {
  openWorksheet: ReturnType<typeof useOpenWorksheet>;
};
export type EditorSessionProjectActions = ReturnType<typeof useProjectOperations>;
export type EditorSessionGraphActions = ReturnType<typeof useGraphManagement>;
export type EditorSessionVariableActions = ReturnType<typeof useVariableManagement>;
export type EditorSessionDataframeActions = ReturnType<typeof useDatabaseManagement>;

export type EditorSessionNodeActions = Pick<
  ReturnType<typeof useNodeManagement>,
  'createNode' | 'deleteNode' | 'deleteNodes'
>;

// ─── Consumer-owned slices ────────────────────────────────────────────────

/** Resource collections used by Detail and sidebar consumers. */
export type EditorSessionResourcesSlice = Pick<
  EditorSessionState,
  'events' | 'functions' | 'variables' | 'dataframes'
>;

/** Detail mutations come directly from their owning command slices. */
export type EditorSessionDetailActionsSlice = Pick<
  EditorSessionVariableActions,
  'updateVariable'
> & Pick<EditorSessionDataframeActions, 'updateDataFrame'>;

// ─── Canvas caller-shaped slices ──────────────────────────────────────────

export type EditorCanvasMode = 'interactive' | 'preview';

export interface EditorCanvasScope {
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: 'event' | 'function';
}

export type EditorCanvasCommandsSlice = Pick<
  EditorSessionHistoryActions,
  | 'copyNodes'
  | 'cutNodes'
  | 'duplicateNodes'
  | 'deleteNodesById'
  | 'breakAllNodeLinks'
  | 'breakConnectionsById'
  | 'selectLinkedNodes'
  | 'disconnectPinById'
  | 'resetPinValue'
  | 'setSelectedNodeIds'
  | 'setSelectedConnectionIds'
> & Pick<
  EditorSessionProjectActions,
  'executeGraph' | 'cancelGraphExecution' | 'clearGraphArtifacts'
> & Pick<EditorSessionNodeActions, 'createNode'>;

export interface EditorCanvasWorkspaceSlice {
  groupId: string;
  activeGraph: {
    graphPath: string;
    kind: 'event' | 'function';
  } | null;
  selectedNodeIds: string[];
  selectedConnectionIds: string[];
}

export type EditorCanvasResourcesSlice = Pick<
  EditorSessionResourcesSlice,
  'variables'
>;

export interface EditorCanvasInteractionSlice {
  contextMenu: EditorSessionState['contextMenu'];
  setContextMenu: EditorSessionLayoutBindings['setContextMenu'];
  pendingConnection: Pin | null;
  setPendingConnection: (pin: Pin | null) => void;
  onCanvasPointerDown: (event: React.PointerEvent) => void;
  onNodePointerDown: (nodeId: string, event: React.PointerEvent) => void;
  onPinPointerDown: (pin: Pin, event: React.PointerEvent) => void;
  insertRerouteAtConnection: (
    connectionId: string,
    position: Readonly<{ x: number; y: number }>,
    graphPath: string,
    groupId: string,
    selection: {
      before: GraphSelection;
      temporary: GraphSelection;
    },
  ) => Promise<CanvasMutationOutcome | false>;
}

export interface EditorCanvasSession {
  commands: EditorCanvasCommandsSlice;
  workspace: EditorCanvasWorkspaceSlice;
  resources: EditorCanvasResourcesSlice;
  interaction: EditorCanvasInteractionSlice;
}

/** EditorSessionProvider 组装命令容器时的 layout 字段提取 */
export function pickEditorSessionLayoutBindings(actions: EditorActions): EditorSessionLayoutBindings {
  return {
    setCanvas: actions.setCanvas,
    setContextMenu: actions.setContextMenu,
    setDetailFocus: actions.setDetailFocus,
    clearDetailFocus: actions.clearDetailFocus,
    activeGroupIdRef: actions.activeGroupIdRef,
    activeTabIdRef: actions.activeTabIdRef,
    viewportRef: actions.viewportRef,
  };
}

export function pickEditorSessionNodeActions(
  nodeMgmt: ReturnType<typeof useNodeManagement>,
): EditorSessionNodeActions {
  return {
    createNode: nodeMgmt.createNode,
    deleteNode: nodeMgmt.deleteNode,
    deleteNodes: nodeMgmt.deleteNodes,
  };
}
