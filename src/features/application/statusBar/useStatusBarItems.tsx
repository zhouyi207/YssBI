import { useEffect, useMemo, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import { useTranslation } from 'react-i18next';
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
} from '@/features/application/editor/editorCommandFocus';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useEditorPaneStateStore } from '@/features/core/dockview/editorPaneStateStore';
import { useDockviewPortSnapshot } from '@/features/core/dockview/useDockviewPortSnapshot';
import { workbenchDockviewPort } from '@/features/core/dockview/workbenchDockviewPort';
import { useExecutionStore } from '@/features/core/execution/useExecutionStore';
import {
  getViewport,
  subscribeToViewport,
  editorViewportScope,
  type ViewportScope,
} from '@/features/core/viewport';
import {
  createBuiltInStatusBarItems,
  useStatusBarSnapshot,
  type StatusBarItemsSnapshot,
  type StatusBarRenderContext,
} from '@/features/core/statusBar';
import { useStatusBarActions } from './useStatusBarActions';
import { useJuliaWorkerStatus } from './useJuliaWorkerStatus';

function formatViewportStatus(scope: ViewportScope | null) {
  if (!scope) return 'X 0 Y 0 100%';
  const viewport = getViewport(scope);
  return `X ${Math.round(viewport.x)} Y ${Math.round(viewport.y)} ${Math.round(viewport.scale * 100)}%`;
}

function ViewportStatus({ scope }: { scope: ViewportScope | null }) {
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {
    if (!scope) return;
    const update = () => {
      if (ref.current) ref.current.textContent = formatViewportStatus(scope);
    };
    update();
    return subscribeToViewport(scope, update);
  }, [scope?.groupId, scope?.graphPath]);

  return <span ref={ref}>{formatViewportStatus(scope)}</span>;
}

export function useStatusBarItems(): StatusBarItemsSnapshot {
  const { t } = useTranslation();
  const actions = useStatusBarActions();
  const juliaWorker = useJuliaWorkerStatus();

  useDockviewPortSnapshot(workbenchDockviewPort);
  const capturedTarget = captureActiveEditorCommandTarget();
  const editorTarget = capturedTarget && isEditorCommandTargetCurrent(capturedTarget)
    ? capturedTarget
    : null;
  const graphTarget = editorTarget
    && (editorTarget.resourceKind === 'event' || editorTarget.resourceKind === 'function')
    ? editorTarget
    : null;
  const selectedNodeIds = useEditorPaneStateStore((state) => (
    graphTarget ? state.selections[graphTarget.panelInstanceId]?.selectedNodeIds : undefined
  ));
  const editor = useMemo(() => ({
    activeEditorGroupId: graphTarget?.groupId ?? null,
    activeTabId: graphTarget?.resourceRef ?? null,
    selectedCount: selectedNodeIds?.length ?? 0,
  }), [graphTarget, selectedNodeIds]);

  const graphStats = useGraphDataStore(
    useShallow((state) => {
      if (!editor.activeTabId) return { nodeCount: 0, connectionCount: 0 };

      const nodeIds = state.getGraphNodeIds(editor.activeTabId);
      const connectionIds = new Set<string>();
      for (const nodeId of nodeIds) {
        for (const pinId of state.getGraphNodePins(editor.activeTabId, nodeId)) {
          for (const connectionId of state.getGraphPinConnections(editor.activeTabId, pinId)) {
            connectionIds.add(connectionId);
          }
        }
      }

      return {
        nodeCount: nodeIds.length,
        connectionCount: connectionIds.size,
      };
    }),
  );

  const executionStatus = useExecutionStore((state) =>
    editor.activeTabId ? state.graphs[editor.activeTabId]?.status ?? 'idle' : 'idle',
  );

  const ctx = useMemo<StatusBarRenderContext>(
    () => ({
      t,
      activeTabId: editor.activeTabId,
      activeEditorGroupId: editor.activeEditorGroupId,
      selectedCount: editor.selectedCount,
      nodeCount: graphStats.nodeCount,
      connectionCount: graphStats.connectionCount,
      executionStatus,
      juliaWorkerState: juliaWorker.state,
      juliaWorkerLabel: juliaWorker.label,
      juliaWorkerTooltip: juliaWorker.tooltip,
    }),
    [t, editor, graphStats, executionStatus, juliaWorker],
  );

  const builtIn = useMemo(
    () => createBuiltInStatusBarItems({
      openLogsPanel: actions.openLogsPanel,
      resetCanvasViewport: actions.resetCanvasViewport,
      executionTooltip: actions.executionTooltip,
      viewportTooltip: actions.viewportTooltip,
      renderViewportStatus: (groupId, graphPath) => (
        <ViewportStatus
          scope={groupId && graphPath ? editorViewportScope(groupId, graphPath) : null}
        />
      ),
    }),
    [
      actions.openLogsPanel,
      actions.resetCanvasViewport,
      actions.executionTooltip,
      actions.viewportTooltip,
    ],
  );

  return useStatusBarSnapshot(ctx, builtIn);
}
