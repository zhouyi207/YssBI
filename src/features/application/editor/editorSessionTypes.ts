/**
 * EditorSession 显式契约 — 按职责切片，避免 `ReturnType<typeof useEditorSessionValue>` 推断链。
 * 新 hook 应依赖命名切片或 `PickEditorSession<…>`，禁止 `...session` 透传未知字段。
 */

import type { RefObject } from 'react';
import type { buildEditorState } from '@/features/core/editor/hooks/useEditorState';
import type { useEditorActions } from '@/features/core/editor/hooks/useEditorActions';
import type { EditorViewport } from '@/features/core/viewport';
import type { LayoutTab } from '@/shared/types/layout/layout';
import type { Pin } from '@/shared/types/domain/pin';
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
  setPendingConnection: EditorActions['setPendingConnection'];
  activeGroupIdRef: RefObject<string | null>;
  activeTabIdRef: RefObject<string | null>;
  viewportRef: RefObject<EditorViewport>;
}

// ─── Command slices（与各 application hook 1:1）────────────────────────────

export type EditorSessionHistoryActions = ReturnType<typeof useEditorOperations>;

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

/** 全窗口 EditorSessionProvider 契约 */
export type EditorSession = EditorSessionState &
  EditorSessionLayoutBindings &
  EditorSessionHistoryActions &
  EditorSessionHistoryAvailability &
  EditorSessionTabActions &
  EditorSessionWorksheetActions &
  EditorSessionProjectActions &
  EditorSessionGraphActions &
  EditorSessionVariableActions &
  EditorSessionDataframeActions &
  EditorSessionNodeActions;

// ─── 常用 Pick 切片（Canvas / Detail / Sidebar 按需依赖）────────────────

export type PickEditorSession<K extends keyof EditorSession> = Pick<EditorSession, K>;

/** 资源集合：variables / events / functions / dataframes */
export type EditorSessionResourcesSlice = PickEditorSession<
  'events' | 'functions' | 'variables' | 'dataframes'
>;

/** Detail 面板资源编辑 */
export type EditorSessionDetailActionsSlice = PickEditorSession<
  'updateVariable' | 'updateDataFrame'
>;

// ─── EditorGroup 叠加层 ───────────────────────────────────────────────────

export interface EditorGroupWorkspaceSlice {
  groupId: string;
  tabs: LayoutTab[];
  activeTabId: string | null;
  selectedNodeIds: string[];
}

export type ConnectPinsHandler = (
  groupId: string,
  pinA: string,
  pinB: string,
) => Promise<void>;

export interface EditorGroupInteractionSlice {
  onCanvasPointerDown: (e: React.PointerEvent) => void;
  onNodePointerDown: (nodeId: string, e: React.PointerEvent) => void;
  onPinPointerDown: (pin: Pin, e: React.PointerEvent) => void;
  connectPins: ConnectPinsHandler;
  setCanvas: (
    updater: EditorViewport | ((prev: EditorViewport) => EditorViewport),
    targetGraphPath?: string,
  ) => void;
}

/** useEditorGroup 返回值：commands + shared + group workspace + 可选 canvas 交互 */
export type EditorGroupSession = EditorSessionResourcesSlice & {
  groups: EditorSession['groups'];
} & ReturnType<typeof import('./useEditorSessionUi').useEditorSessionUi> &
  Pick<
    EditorSession,
    | keyof EditorSessionLayoutBindings
    | keyof EditorSessionHistoryActions
    | keyof EditorSessionTabActions
    | keyof EditorSessionWorksheetActions
    | keyof EditorSessionProjectActions
    | keyof EditorSessionGraphActions
    | keyof EditorSessionVariableActions
    | keyof EditorSessionDataframeActions
    | keyof EditorSessionNodeActions
  > &
  EditorGroupWorkspaceSlice &
  EditorGroupInteractionSlice;

/** 唯一允许的 session 合并点（useEditorGroup） */
export function composeEditorGroupSession(
  shared: EditorSessionResourcesSlice & { groups: EditorSession['groups'] },
  ui: ReturnType<typeof import('./useEditorSessionUi').useEditorSessionUi>,
  commands: Pick<
    EditorSession,
    keyof EditorSessionLayoutBindings | keyof EditorSessionHistoryActions
  > &
    EditorSessionTabActions &
    EditorSessionWorksheetActions &
    EditorSessionProjectActions &
    EditorSessionGraphActions &
    EditorSessionVariableActions &
    EditorSessionDataframeActions &
    EditorSessionNodeActions,
  workspace: EditorGroupWorkspaceSlice,
  interaction: EditorGroupInteractionSlice,
): EditorGroupSession {
  return Object.assign({}, shared, ui, commands, workspace, interaction);
}

/** useEditorSessionValue 组装时的 layout 字段提取 */
export function pickEditorSessionLayoutBindings(actions: EditorActions): EditorSessionLayoutBindings {
  return {
    setCanvas: actions.setCanvas,
    setContextMenu: actions.setContextMenu,
    setDetailFocus: actions.setDetailFocus,
    clearDetailFocus: actions.clearDetailFocus,
    setPendingConnection: actions.setPendingConnection,
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

export const EDITOR_SESSION_RESOURCE_KEYS = [
  'events',
  'functions',
  'variables',
  'dataframes',
] as const satisfies readonly (keyof EditorSessionResourcesSlice)[];

export function pickEditorSessionResources(session: EditorSession): EditorSessionResourcesSlice {
  return {
    events: session.events,
    functions: session.functions,
    variables: session.variables,
    dataframes: session.dataframes,
  };
}
