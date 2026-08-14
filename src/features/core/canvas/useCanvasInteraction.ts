import React, { useCallback, useEffect, useMemo, useRef } from 'react';
import { getGraphByPath } from '@/features/core/dataStore';
import {
  getActiveLayoutTab,
  getEditorGroupGraphSelection,
  updateEditorGroupSelectedConnectionIds,
  updateEditorGroupSelectedNodeIds,
} from '@/features/core/layout/layoutTabQueries';
import type { GraphSelection } from '@/features/core/layout';

import { persistGraphViewport, editorViewportScope } from '@/features/core/viewport';
import { useEditorStore } from '@/features/core/editor';
import { getCanvasInteraction, useGraphInteractionStore } from '@/features/core/graphInteraction/graphInteractionStore';
import { executeCommand } from '@/features/core/history';
import type { Pin } from '@/shared/types/domain';
import type { PinData } from '@/shared/types/store/graph';
import type { EditorViewport } from '@/features/core/viewport';
import { logger } from '@/utils/appLogger';
import { getPinCompatibility } from '@/shared/utils/pinCompatibility';

import { getCanvasWorldPoint, resolveTabId } from './canvasInteractionUtils';
import { attachCanvasPointerLoop, registerCanvasPointerScope } from './canvasPointerLoop';
import { cancelCanvasInteraction, startCanvasInteraction } from './canvasInteractionCleanup';
import type { CanvasInteractionHandlers } from './canvasMutationContracts';

export type { CanvasInteractionHandlers } from './canvasMutationContracts';

type PinPointerAction = 'none' | 'disconnect' | 'move' | 'draw';

interface DoubleClickSelectionSnapshot {
  before: GraphSelection;
  temporary: GraphSelection;
}

function selectionMatches(
  actual: GraphSelection,
  expected: GraphSelection,
): boolean {
  return actual.nodeIds.size === expected.nodeIds.size
    && [...actual.nodeIds].every((id) => expected.nodeIds.has(id))
    && actual.connectionIds.size === expected.connectionIds.size
    && [...actual.connectionIds].every((id) => expected.connectionIds.has(id));
}

function restoreGraphSelection(groupId: string, selection: GraphSelection): void {
  if (selection.nodeIds.size > 0) {
    updateEditorGroupSelectedNodeIds([...selection.nodeIds], groupId);
  } else {
    updateEditorGroupSelectedConnectionIds([...selection.connectionIds], groupId);
  }
}

type PinPointerModifiers = Pick<
  MouseEvent,
  'button' | 'altKey' | 'ctrlKey' | 'metaKey'
>;

export function resolvePinPointerAction(
  event: Partial<PinPointerModifiers> & Pick<PinPointerModifiers, 'button'>,
  capability: Pick<NonNullable<PinData['connections']>, 'current' | 'canAppend' | 'canReplace' | 'canMove'>,
): PinPointerAction {
  if (event.button !== 0) return 'none';
  if (event.altKey) return capability.canMove ? 'disconnect' : 'none';
  if (event.ctrlKey || event.metaKey) {
    return capability.canMove && capability.current > 0 ? 'move' : 'none';
  }
  return capability.canAppend || capability.canReplace ? 'draw' : 'none';
}

interface UseCanvasInteractionProps {
  activeGroupIdRef: React.RefObject<string>;
  activeTabIdRef: React.RefObject<string | null>;
  viewportRef: React.RefObject<EditorViewport>;
  setSelectedNodeIds: (updater: string[] | ((prev: string[]) => string[]), targetGroupId?: string) => void;
  handlers: CanvasInteractionHandlers;
  enabled?: boolean;
  uiEnabled?: boolean;
}

export function useCanvasInteraction({
  activeGroupIdRef,
  activeTabIdRef,
  viewportRef,
  setSelectedNodeIds,
  handlers,
  enabled = true,
  uiEnabled = enabled,
}: UseCanvasInteractionProps) {
  const contextMenu = useEditorStore((state) => uiEnabled ? state.contextMenu : null);
  const setContextMenu = useEditorStore((state) => state.setContextMenu);
  const activeGraphPath = activeTabIdRef.current;
  const pendingConnection = useGraphInteractionStore((state) => {
    if (!activeGraphPath) return null;
    const interaction = getCanvasInteraction(state, activeGraphPath, activeGroupIdRef.current);
    return interaction.type === 'pendingNodeCreation'
      ? interaction.session.source as Pin
      : null;
  });
  const setSelectedNodeIdsRef = useRef(setSelectedNodeIds);
  setSelectedNodeIdsRef.current = setSelectedNodeIds;

  const persistViewport = useCallback((pane?: { groupId: string; graphPath: string } | null) => {
    const groupId = pane?.groupId ?? activeGroupIdRef.current;
    const graphPath = pane?.graphPath ?? activeTabIdRef.current;
    if (groupId && graphPath) persistGraphViewport(editorViewportScope(groupId, graphPath));
  }, [activeGroupIdRef, activeTabIdRef]);

  const setPendingConnection = useCallback((pin: Pin | null) => {
    const groupId = activeGroupIdRef.current;
    const graphPath = resolveTabId(groupId, activeTabIdRef);
    if (!graphPath) return;
    if (!pin) {
      cancelCanvasInteraction(graphPath, groupId);
      return;
    }
    const menu = useEditorStore.getState().contextMenu;
    startCanvasInteraction(graphPath, {
      type: 'pendingNodeCreation',
      session: {
        groupId,
        graphPath,
        source: pin as PinData,
        screenX: menu?.x ?? 0,
        screenY: menu?.y ?? 0,
      },
    });
  }, [activeGroupIdRef, activeTabIdRef]);

  const connectPins = useCallback(async (groupId: string, pinA: string, pinB: string) => {
    const graphPath = resolveTabId(groupId, activeTabIdRef);
    if (!graphPath) return;

    const graph = getGraphByPath(graphPath);
    const first = graph?.pins.find((pin) => pin.id === pinA);
    const second = graph?.pins.find((pin) => pin.id === pinB);
    if (first && second) {
      const source = first.direction === 'output' ? first : second;
      const target = first.direction === 'input' ? first : second;
      if (getPinCompatibility(source, target) === 'incompatible') {
        logger.graph.warn('Ignored type-mismatched pin connection attempt', 'CanvasInteraction');
        return;
      }
    }

    const applied = await executeCommand(graphPath, 'ConnectPins', { pinA, pinB });
    if (!applied) logger.graph.error('Failed to connect ports', 'CanvasInteraction');
  }, [activeTabIdRef]);

  const insertRerouteAtConnection = useCallback(async (
    connectionId: string,
    position: Readonly<{ x: number; y: number }>,
    graphPath: string,
    groupId: string,
    selection: DoubleClickSelectionSnapshot,
  ) => {
    if (getActiveLayoutTab(groupId)?.activeTabId !== graphPath) return false;
    let outcome;
    try {
      outcome = await handlers.insertRerouteAtConnection({ graphPath, connectionId, position });
    } catch {
      outcome = false as const;
    }
    if (getActiveLayoutTab(groupId)?.activeTabId !== graphPath
      || !selectionMatches(getEditorGroupGraphSelection(groupId), selection.temporary)) return outcome;

    if (outcome !== false && outcome.status === 'applied') {
      updateEditorGroupSelectedConnectionIds(
        [...selection.temporary.connectionIds].filter((id) => id !== connectionId),
        groupId,
      );
    } else {
      restoreGraphSelection(groupId, selection.before);
    }
    return outcome;
  }, [handlers]);

  const onCanvasPointerDown = useCallback((event: React.PointerEvent, groupId?: string) => {
    const gid = groupId ?? activeGroupIdRef.current;
    const graphPath = resolveTabId(gid, activeTabIdRef);
    if (!graphPath) return;
    if (event.button === 1 || event.button === 2 || (event.button === 0 && event.altKey)) {
      registerCanvasPointerScope({ graphPath, groupId: gid });
      startCanvasInteraction(graphPath, {
        type: 'panning',
        session: { groupId: gid, startX: event.clientX, startY: event.clientY, lastX: event.clientX, lastY: event.clientY, moved: false },
      });
    } else if (event.button === 0) {
      registerCanvasPointerScope({ graphPath, groupId: gid });
      startCanvasInteraction(graphPath, {
        type: 'selecting',
        session: { groupId: gid, startX: event.clientX, startY: event.clientY, currentX: event.clientX, currentY: event.clientY, preserveSelection: event.shiftKey },
      });
    }
  }, [activeGroupIdRef, activeTabIdRef]);

  const onNodePointerDown = useCallback((nodeId: string, event: React.PointerEvent, groupId?: string) => {
    event.stopPropagation();
    if (event.button !== 0) return;
    const gid = groupId ?? activeGroupIdRef.current;
    const graphPath = resolveTabId(gid, activeTabIdRef);
    if (!graphPath) return;
    const selected = [...getEditorGroupGraphSelection(gid).nodeIds];
    const toggleSelection = event.shiftKey || event.ctrlKey || event.metaKey;
    const nodeIds = toggleSelection
      ? (selected.includes(nodeId)
        ? selected.filter((id) => id !== nodeId)
        : [...selected, nodeId])
      : [nodeId];
    setSelectedNodeIdsRef.current(nodeIds, gid);
    registerCanvasPointerScope({ graphPath, groupId: gid });
    startCanvasInteraction(graphPath, {
      type: 'draggingNodes',
      session: { groupId: gid, nodeId, lastX: event.clientX, lastY: event.clientY, moved: false, nodeIds, delta: { x: 0, y: 0 } },
    });
  }, [activeGroupIdRef, activeTabIdRef]);

  const onPinPointerDown = useCallback(async (pin: Pin, event: React.PointerEvent, groupId?: string) => {
    event.stopPropagation();
    if (event.button !== 0) return;
    const gid = groupId ?? activeGroupIdRef.current;
    const graphPath = resolveTabId(gid, activeTabIdRef);
    const projected = pin as PinData;
    if (!graphPath || !projected.connections || !projected.address) return;
    const action = resolvePinPointerAction(event, projected.connections);
    if (action === 'disconnect') {
      await handlers.disconnectPort(graphPath, projected.id);
      return;
    }
    if (action === 'none') return;
    const world = getCanvasWorldPoint(gid, graphPath, event.clientX, event.clientY);
    registerCanvasPointerScope({ graphPath, groupId: gid });
    startCanvasInteraction(graphPath, {
      type: action === 'move' ? 'movingConnections' : 'drawingConnection',
      session: {
        groupId: gid,
        graphPath,
        source: projected,
        screenX: event.clientX,
        screenY: event.clientY,
        worldX: world.x,
        worldY: world.y,
        hoveredTarget: null,
        snappedTarget: null,
        snappedWorld: null,
        feedback: null,
      },
    });
  }, [activeGroupIdRef, activeTabIdRef, handlers]);

  useEffect(() => {
    if (!enabled) return;
    return attachCanvasPointerLoop({
      activeGroupIdRef,
      activeTabIdRef,
      viewportRef,
      setSelectedNodeIds: (updater, groupId) => setSelectedNodeIdsRef.current(updater, groupId),
      persistViewport,
      setContextMenu,
      submitConnection: handlers.submitConnection,
      reportMutationFailure: handlers.reportMutationFailure,
    });
  }, [enabled, activeGroupIdRef, activeTabIdRef, viewportRef, persistViewport, setContextMenu, handlers]);

  return useMemo(() => ({
    contextMenu: uiEnabled ? contextMenu : null,
    setContextMenu,
    pendingConnection: uiEnabled ? pendingConnection : null,
    setPendingConnection,
    connectPins,
    insertRerouteAtConnection,
    onCanvasPointerDown,
    onNodePointerDown,
    onPinPointerDown,
  }), [uiEnabled, contextMenu, setContextMenu, pendingConnection, setPendingConnection, connectPins, insertRerouteAtConnection, onCanvasPointerDown, onNodePointerDown, onPinPointerDown]);
}
