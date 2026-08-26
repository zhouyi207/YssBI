import { useCallback, useMemo, useRef } from 'react';
import { useShallow } from 'zustand/react/shallow';
import {
  useCanvasDrop,
  useCanvasOverlayHandlers,
  useCanvasViewport,
  useCanvasWheelZoom,
  useEditorCanvas,
} from '@/features/application/editor';
import type { EditorCanvasScope } from '@/features/application/editor';
import { CanvasContextMenuProvider } from '@/features/application/editor/CanvasContextMenuContext';
import type { CanvasContextMenuActions } from '@/features/application/editor/CanvasContextMenuContext';
import { useNodeDragPreview } from '@/features/core/canvas/useNodeDragPreview';
import { useSelectionBoxPreview } from '@/features/core/canvas/useSelectionBoxPreview';
import { useGraphDataStore } from '@/features/core/dataStore';
import { useExecutionVisualBinder } from '@/features/core/execution';
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from '@/features/core/graphInteraction/graphInteractionStore';
import { editorViewportScope } from '@/features/core/viewport';
import type { NodeCreationDescriptor } from '@/features/domain/nodeCatalog/creationDescriptor';
import type { PortAddressDto } from '@/shared/types/dto/editorProjection';
import { CanvasNode } from '../../Nodes/CanvasNode';
import CanvasOverlays, { type CanvasOverlaysModel } from '../overlays/CanvasOverlays';
import { ConnectionLine } from './ConnectionLine';
import { EdgesOverlay } from './EdgesOverlay';
import { TransformContainer } from './TransformContainer';
import { ViewportGrid } from './ViewportGrid';

const EMPTY_NODE_IDS: string[] = [];

export interface CanvasProps {
  mode: 'interactive' | 'preview';
  panelInstanceId: string;
  groupId: string;
  graphPath: string;
  graphKind: 'event' | 'function';
}

export default function Canvas({
  mode,
  panelInstanceId,
  groupId,
  graphPath,
  graphKind,
}: CanvasProps) {
  const interactive = mode === 'interactive';
  const scope = useMemo<EditorCanvasScope>(
    () => ({ panelInstanceId, groupId, graphPath, graphKind }),
    [graphKind, graphPath, groupId, panelInstanceId],
  );
  const {
    commands: {
      copyNodes,
      cutNodes,
      duplicateNodes,
      deleteNodesById,
      breakAllNodeLinks,
      breakConnectionsById,
      selectLinkedNodes,
      disconnectPinById,
      resetPinValue,
      setSelectedNodeIds,
      setSelectedConnectionIds,
      executeGraph,
      cancelGraphExecution,
      clearGraphArtifacts,
      createNode,
    },
    workspace: {
      activeGraph,
      selectedNodeIds,
      selectedConnectionIds,
    },
    resources: { variables },
    interaction: {
      onCanvasPointerDown,
      onNodePointerDown,
      onPinPointerDown,
      contextMenu,
      setContextMenu,
      pendingConnection,
      setPendingConnection,
      insertRerouteAtConnection,
    },
  } = useEditorCanvas({ mode, scope });
  const activeTabId = activeGraph?.graphPath ?? null;

  const gesturePinData = useGraphInteractionStore((state) => {
    if (!interactive || !activeTabId) return null;
    const interaction = getCanvasInteraction(state, activeTabId, groupId);
    return interaction.type === 'drawingConnection' || interaction.type === 'movingConnections'
      ? interaction.session.source
      : null;
  });

  const canvasElementRef = useRef<HTMLDivElement>(null);
  const selectionBoxRef = useRef<HTMLDivElement>(null);

  const viewportScope = useMemo(
    () => (activeTabId ? editorViewportScope(groupId, activeTabId) : null),
    [activeTabId, groupId],
  );

  useNodeDragPreview(canvasElementRef, interactive ? groupId : null, interactive ? activeTabId : null);
  useSelectionBoxPreview(
    selectionBoxRef,
    canvasElementRef,
    interactive ? activeTabId ?? undefined : undefined,
    interactive ? groupId : undefined,
  );
  useExecutionVisualBinder(canvasElementRef, interactive ? activeTabId ?? undefined : undefined);

  const selectedNodeIdsSet = useMemo(
    () => new Set(selectedNodeIds),
    [selectedNodeIds],
  );

  const graphNodeIds = useGraphDataStore(
    useShallow((state) => (activeTabId ? state.getGraphNodeIds(activeTabId) : EMPTY_NODE_IDS)),
  );
  const graphRevision = useGraphDataStore((state) => (
    interactive && activeTabId
      ? state.graphEntities[activeTabId]?.sourceRevision ?? null
      : null
  ));

  const { getPinWorldPos, getCanvasLocalPoint } = useCanvasViewport(
    canvasElementRef,
    groupId,
    activeTabId,
  );
  useCanvasWheelZoom(canvasElementRef, viewportScope, interactive);

  const {
    variableDropMenu,
    setVariableDropMenu,
    handleNodeAddInput,
    handleNodeRemovePin,
    handleContextMenu,
    handleVariableDropGet,
    handleVariableDropSet,
  } = useCanvasDrop({
    canvasElementRef,
    panelInstanceId,
    groupId,
    graphPath: interactive ? activeTabId : null,
    variables,
    setContextMenu,
    setPendingConnection,
    createNode,
    enabled: interactive,
  });

  const { handleNodePaletteSelect } = useCanvasOverlayHandlers({
    canvasElementRef,
    panelInstanceId,
    groupId,
    activeTabId,
    pendingConnection,
    setContextMenu,
    setPendingConnection,
  });

  const activePin = useMemo(() => {
    if (!interactive) return null;
    if (gesturePinData) return gesturePinData;
    if (pendingConnection && contextMenu?.visible) return pendingConnection;
    return null;
  }, [contextMenu, gesturePinData, interactive, pendingConnection]);

  const sourcePort = pendingConnection && 'address' in pendingConnection
    ? (pendingConnection as typeof pendingConnection & { address?: PortAddressDto }).address ?? null
    : null;

  const handlePaletteSelect = useCallback((descriptor: NodeCreationDescriptor, locale: string) => {
    if (contextMenu?.visible) {
      void handleNodePaletteSelect(descriptor, locale, contextMenu);
    }
  }, [contextMenu, handleNodePaletteSelect]);

  const handleSelectedConnectionIdsChange = useCallback((
    connectionIds: string[],
    graphPath: string,
    targetGroupId: string,
  ) => {
    if (graphPath === activeTabId && targetGroupId === groupId) {
      setSelectedConnectionIds(connectionIds, targetGroupId);
    }
  }, [activeTabId, groupId, setSelectedConnectionIds]);

  const closePalette = useCallback(() => {
    setContextMenu(null);
    setPendingConnection(null);
  }, [setContextMenu, setPendingConnection]);

  const overlayModel = useMemo((): CanvasOverlaysModel => ({
    graph: activeGraph
      ? { kind: activeGraph.kind, graphPath: activeGraph.graphPath }
      : { kind: 'unavailable' },
    palette: contextMenu?.visible
      ? {
          kind: 'visible',
          x: contextMenu.x,
          y: contextMenu.y,
          graphPath: activeTabId,
          graphRevision,
          sourcePort,
          onSelect: handlePaletteSelect,
          onClose: closePalette,
        }
      : { kind: 'hidden' },
    variable: variableDropMenu
      ? {
          kind: 'visible',
          x: variableDropMenu.x,
          y: variableDropMenu.y,
          variableName: variableDropMenu.variableName,
          onGet: () => { void handleVariableDropGet(variableDropMenu); },
          onSet: () => { void handleVariableDropSet(variableDropMenu); },
          onClose: () => setVariableDropMenu(null),
        }
      : { kind: 'hidden' },
    execution: activeGraph?.kind === 'event'
      ? {
          kind: 'event',
          graphPath: activeGraph.graphPath,
          onExecute: () => { void executeGraph(activeGraph.graphPath); },
          onCancelExecution: () => { void cancelGraphExecution(activeGraph.graphPath); },
          onClearArtifacts: () => { void clearGraphArtifacts(activeGraph.graphPath); },
        }
      : { kind: 'hidden' },
  }), [
    activeGraph,
    activeTabId,
    cancelGraphExecution,
    clearGraphArtifacts,
    closePalette,
    contextMenu,
    executeGraph,
    graphRevision,
    handlePaletteSelect,
    handleVariableDropGet,
    handleVariableDropSet,
    setVariableDropMenu,
    sourcePort,
    variableDropMenu,
  ]);

  const isDraggingPin = activePin != null;
  const contextMenuActions = useMemo((): CanvasContextMenuActions => ({
    selectNode: (nodeId, targetGroupId) => setSelectedNodeIds([nodeId], targetGroupId ?? groupId),
    copyNode: (nodeId) => copyNodes([nodeId]),
    cutNode: (nodeId) => cutNodes([nodeId]),
    duplicateNode: (nodeId) => duplicateNodes([nodeId]),
    deleteNode: (nodeId) => deleteNodesById([nodeId]),
    breakAllNodeLinks,
    selectLinkedNodes,
    disconnectPin: disconnectPinById,
    resetPinValue,
    removeRepeatablePin: handleNodeRemovePin,
  }), [
    breakAllNodeLinks,
    copyNodes,
    cutNodes,
    deleteNodesById,
    disconnectPinById,
    duplicateNodes,
    groupId,
    handleNodeRemovePin,
    resetPinValue,
    selectLinkedNodes,
    setSelectedNodeIds,
  ]);

  const canvasElement = (
      <div
        ref={canvasElementRef}
        data-editor-panel-instance-id={panelInstanceId}
        data-editor-graph-path={activeTabId ?? undefined}
        data-editor-graph-kind={graphKind}
        className="relative w-full h-full overflow-hidden bg-[var(--workbench-bg)] select-none"
      >
        <ViewportGrid viewportScope={viewportScope} />

        <div
          className="absolute inset-0"
          onPointerDown={onCanvasPointerDown}
          onContextMenu={interactive ? handleContextMenu : undefined}
        >
          <ConnectionLine
            viewportScope={viewportScope}
            getPinWorldPos={getPinWorldPos}
            getCanvasLocalPoint={getCanvasLocalPoint}
            pendingConnection={interactive ? pendingConnection : null}
            menuPos={interactive ? contextMenu : null}
          />

          <TransformContainer viewportScope={viewportScope}>
            {mode === 'interactive' ? (
              <EdgesOverlay
                mode="interactive"
                graphPath={activeTabId ?? ''}
                groupId={groupId}
                getPinWorldPos={getPinWorldPos}
                getCanvasLocalPoint={getCanvasLocalPoint}
                dimmed={isDraggingPin}
                selectedNodeIds={selectedNodeIds}
                selectedConnectionIds={selectedConnectionIds}
                onSelectedConnectionIdsChange={handleSelectedConnectionIdsChange}
                onBreakConnections={breakConnectionsById}
                onEdgeDoubleClick={insertRerouteAtConnection}
              />
            ) : (
              <EdgesOverlay
                mode="preview"
                graphPath={activeTabId ?? ''}
                groupId={groupId}
                getPinWorldPos={getPinWorldPos}
                getCanvasLocalPoint={getCanvasLocalPoint}
                dimmed={isDraggingPin}
              />
            )}
            {graphNodeIds.map((nodeId) => (
              <CanvasNode
                key={nodeId}
                id={nodeId}
                graphPath={activeTabId ?? undefined}
                groupId={groupId}
                selected={interactive && selectedNodeIdsSet.has(nodeId)}
                activePin={activePin}
                onPointerDown={onNodePointerDown}
                onAddInput={handleNodeAddInput}
                onRemovePin={handleNodeRemovePin}
                onPinPointerDown={onPinPointerDown}
              />
            ))}
          </TransformContainer>
        </div>

        <div ref={selectionBoxRef} aria-hidden />

        {mode === 'interactive' ? <CanvasOverlays model={overlayModel} /> : null}
      </div>
  );

  return interactive
    ? <CanvasContextMenuProvider value={contextMenuActions}>{canvasElement}</CanvasContextMenuProvider>
    : canvasElement;
}
