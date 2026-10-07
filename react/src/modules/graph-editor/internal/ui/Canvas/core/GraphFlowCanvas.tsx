import {
  useCallback,
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  ConnectionMode,
  useStore,
  useStoreApi,
  useConnection,
  type Connection,
  type EdgeMouseHandler,
  type NodeChange,
  type NodeSelectionChange,
  type NodeMouseHandler,
  type OnConnectStart,
  type OnConnectEnd,
  type OnMove,
} from "@xyflow/react";
import type {
  EditorCanvasSession,
  EditorCanvasViewportSession,
  GraphContextMenuActions,
} from "@/features/application/editor";
import { useGraphRead } from "@/features/core/graph/read";
import { useConnectionCandidates } from "@/features/application/graphEditing/useConnectionCandidates";
import type { ConnectionIntent } from "@/shared/types/domain/connectionCandidates";
import {
  EDITOR_VIEWPORT_SCALE_LIMITS,
  type EditorViewport,
} from "@/features/core/viewport/editorViewport";
import { ConnectionContextMenu } from "../../ContextMenu";
import { portAddressKey } from "@/features/domain/editorProjection";
import {
  GraphFlowContext,
  GraphFlowInteractionContext,
  createGraphFlowInteractionStore,
} from "./GraphFlowContext";
import { GraphFlowNode } from "./GraphFlowNode";
import { GraphFlowEdge } from "./GraphFlowEdge";
import { useGraphFlowNodes } from "./useGraphFlowNodes";
import { GraphFlowConnection, PendingFlowConnection } from "./GraphFlowConnection";
import {
  createGraphFlowModelProjector,
  createGraphFlowEdgeViewProjector,
  getFlowPinFeedback,
  projectFlowInteraction,
  resolveFlowPinAction,
  type FlowInteractionProjection,
  type GraphFlowNode as FlowNode,
  type GraphFlowEdge as FlowEdge,
} from "./graphFlowModel";
import "@xyflow/react/dist/base.css";
import {
  flowCanvasInteractionProps,
  synchronizeFlowViewport,
} from "@/shared/ui/flowCanvasInteraction";
import "./graphFlow.css";

const nodeTypes = { graph: GraphFlowNode };
const edgeTypes = { graph: GraphFlowEdge };
type GestureLease = NonNullable<ReturnType<EditorCanvasSession["interaction"]["beginGesture"]>>;
type SelectionPointerSnapshot = {
  nodeIds: string[];
  connectionIds: string[];
  shiftKey: boolean;
};

interface GraphFlowCanvasProps {
  panelInstanceId: string;
  graphPath: string;
  groupId: string;
  interactive: boolean;
  contextMenuActions: GraphContextMenuActions | null;
  canvas: EditorCanvasSession;
  viewport: EditorCanvasViewportSession;
  onContextMenu(event: ReactMouseEvent | MouseEvent): void;
}

export function GraphFlowCanvas(props: GraphFlowCanvasProps) {
  return (
    <ReactFlowProvider
      key={JSON.stringify([props.panelInstanceId, props.groupId, props.graphPath])}
    >
      <GraphFlowRuntime {...props} />
    </ReactFlowProvider>
  );
}

function clientPoint(event: MouseEvent | TouchEvent) {
  const point = "changedTouches" in event ? event.changedTouches[0] : event;
  return point ? { x: point.clientX, y: point.clientY } : null;
}

// Target changes only publish local decorations, without rerendering the canvas adapter.
function GraphFlowFeedback({
  store,
  feedback,
}: {
  store: ReturnType<typeof createGraphFlowInteractionStore>;
  feedback: FlowInteractionProjection;
}) {
  const targetId = useConnection((connection) => connection.toHandle?.id ?? null);
  const target = targetId ? getFlowPinFeedback(feedback, targetId) : null;
  const displacedIds = target?.kind === "replace" ? target.displacedConnectionIds : undefined;
  const replacedConnectionIds = useMemo(() => new Set(displacedIds), [displacedIds]);
  useLayoutEffect(() => {
    store.setState({ ...feedback, targetId, replacedConnectionIds }, true);
  }, [store, feedback, targetId, replacedConnectionIds]);
  return null;
}

function GraphFlowRuntime({
  graphPath,
  groupId,
  panelInstanceId,
  interactive,
  canvas,
  contextMenuActions,
  viewport,
  onContextMenu,
}: GraphFlowCanvasProps) {
  const flowStore = useStoreApi<FlowNode, FlowEdge>();
  const panZoom = useStore((state) => state.panZoom);
  const [defaultViewport] = useState(() => {
    const initial = viewport.getViewport();
    return { x: initial.x, y: initial.y, zoom: initial.scale };
  });
  const synchronizeViewport = useCallback(
    (next: EditorViewport) => {
      synchronizeFlowViewport(flowStore, { x: next.x, y: next.y, zoom: next.scale });
    },
    [flowStore],
  );
  useLayoutEffect(
    () => viewport.subscribe(synchronizeViewport),
    // Reapply navigation received before React Flow initialized its pan/zoom instance.
    [viewport, synchronizeViewport, panZoom],
  );
  const projectModel = useMemo(createGraphFlowModelProjector, []);
  const model = useGraphRead((snapshot) => projectModel(snapshot.graphEntities[graphPath]));
  const modelRef = useRef(model);
  modelRef.current = model;
  const { interaction, commands, workspace } = canvas;
  const pan = useRef<GestureLease | null>(null);
  const contextMenuPress = useRef<{
    x: number;
    y: number;
    moved: boolean;
    released: boolean;
  } | null>(null);
  const selectionGesture = useRef<GestureLease | null>(null);
  const selectionPointer = useRef<SelectionPointerSnapshot | null>(null);
  const suppressClick = useRef(false);
  const onNodeDragCancel = useCallback(() => {
    suppressClick.current = true;
  }, []);
  const { updateNodes, startNodeDrag, stopNodeDrag, isDragging, recoverCancelledDrag } =
    useGraphFlowNodes({
      graphPath,
      model,
      selectedNodeIds: workspace.selectedNodeIds,
      interactive,
      interaction,
      onCancel: onNodeDragCancel,
    });
  const connection = useRef<{
    lease: GestureLease;
    sourceId: string;
    intent: ConnectionIntent;
    start: { x: number; y: number };
    submitted: boolean;
  } | null>(null);
  const [connectionSource, setConnectionSource] = useState<{
    sourceId: string;
    intent: ConnectionIntent;
  } | null>(null);
  const [edgeMenu, setEdgeMenu] = useState<{ x: number; y: number; ids: string[] } | null>(null);
  const beforeEdgeClick = useRef<{
    id: string;
    before: { nodeIds: Set<string>; connectionIds: Set<string> };
    temporary: { nodeIds: Set<string>; connectionIds: Set<string> };
  } | null>(null);
  useEffect(() => {
    return () => {
      connection.current?.lease.finish();
      pan.current?.finish();
      selectionGesture.current?.finish();
    };
  }, []);
  useEffect(() => {
    if (!interactive) setEdgeMenu(null);
  }, [interactive]);
  const projectEdgeViews = useMemo(createGraphFlowEdgeViewProjector, []);
  const selectedConnectionIds = useMemo(
    () => new Set(workspace.selectedConnectionIds),
    [workspace.selectedConnectionIds],
  );
  const edges = projectEdgeViews(model.edges, selectedConnectionIds, interactive);
  const pendingSourceId = interaction.pendingConnection
    ? portAddressKey(interaction.pendingConnection)
    : undefined;
  const pendingSource = pendingSourceId ? model.pins[pendingSourceId] : undefined;
  const sourceId = connectionSource?.sourceId ?? pendingSourceId;
  const candidates = useConnectionCandidates({
    graphPath,
    sourcePort: sourceId ? (model.pins[sourceId]?.address ?? null) : null,
    intent: connectionSource?.intent ?? "connect",
    enabled: interactive && !!sourceId,
  });
  const candidatesRef = useRef(candidates);
  candidatesRef.current = candidates;
  const [connectionStore] = useState(createGraphFlowInteractionStore);
  const feedback = useMemo(
    () => projectFlowInteraction(model.pins, model.nodeIds, sourceId, candidates.decisions),
    [model.pins, model.nodeIds, sourceId, candidates.decisions],
  );
  const context = useMemo(
    () => ({ graphPath, groupId, interactive, contextMenuActions }),
    [graphPath, groupId, interactive, contextMenuActions],
  );

  const resetSelectionOverlay = useCallback(() => {
    flowStore.setState({
      userSelectionActive: false,
      userSelectionRect: null,
      nodesSelectionActive: false,
    });
  }, [flowStore]);
  const onNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      let selectionChanges: NodeSelectionChange[] | undefined;
      for (const change of changes) {
        if (change.type === "select") (selectionChanges ??= []).push(change);
      }
      if (
        selectionChanges &&
        interaction.isInteractive() &&
        (workspace.selectedNodeIds.length > 0 ||
          selectionChanges.some((change) => change.selected)) &&
        (!selectionGesture.current || selectionGesture.current.isCurrent())
      ) {
        commands.setSelectedNodeIds((previous) => {
          const next = new Set(previous);
          for (const change of selectionChanges) {
            if (change.selected) next.add(change.id);
            else next.delete(change.id);
          }
          return [...next];
        }, groupId);
      }
      updateNodes(changes);
    },
    [
      commands.setSelectedNodeIds,
      groupId,
      interaction.isInteractive,
      workspace.selectedNodeIds.length,
      updateNodes,
    ],
  );

  const onConnectStart: OnConnectStart = useCallback(
    (event, { handleId }) => {
      const pin = handleId ? modelRef.current.pins[handleId] : null;
      const point = clientPoint(event);
      if (!pin || !point) return;
      const action = "button" in event ? resolveFlowPinAction(event, pin) : "connect";
      if (action !== "connect" && action !== "moveConnections") return;
      const lease = interaction.beginGesture(
        action === "connect" ? "drawingConnection" : "movingConnections",
        () => {
          flowStore.getState().cancelConnection();
          setConnectionSource(null);
        },
      );
      if (!lease) {
        flowStore.getState().cancelConnection();
        return;
      }
      connection.current = {
        lease,
        sourceId: pin.id,
        intent: action,
        start: point,
        submitted: false,
      };
      setConnectionSource({ sourceId: pin.id, intent: action });
    },
    [interaction.beginGesture, flowStore],
  );
  const connectionTarget = useCallback((candidate: Connection | FlowEdge) => {
    const current = connection.current;
    if (!current?.lease.isCurrent()) return null;
    const targetId =
      candidate.sourceHandle === current.sourceId ? candidate.targetHandle : candidate.sourceHandle;
    return targetId ? { current, targetId } : null;
  }, []);
  const isValidConnection = useCallback(
    (candidate: Connection | FlowEdge) => {
      const target = connectionTarget(candidate);
      return (
        !!target &&
        // A pending preview is neutral. Fast drops still go through Rust's
        // authoritative mutation validation, including when a query failed.
        candidatesRef.current.decisionFor(
          target.current.sourceId,
          target.current.intent,
          target.targetId,
        )?.kind !== "invalid"
      );
    },
    [connectionTarget],
  );
  const onConnect = useCallback(
    (candidate: Connection) => {
      const target = connectionTarget(candidate);
      if (!target || !isValidConnection(candidate) || target.current.submitted) return;
      target.current.submitted = true;
      void interaction.mutations
        .submitConnection({
          graphPath,
          intent: target.current.intent,
          sourcePinId: target.current.sourceId,
          targetPinId: target.targetId,
        })
        .then((outcome) => {
          if (outcome.status === "failed")
            interaction.mutations.reportMutationFailure({
              graphPath,
              intent: target.current.intent,
            });
        })
        .catch(() =>
          interaction.mutations.reportMutationFailure({ graphPath, intent: target.current.intent }),
        );
    },
    [connectionTarget, graphPath, interaction.mutations, isValidConnection],
  );
  const onConnectEnd: OnConnectEnd = useCallback(
    (event, state) => {
      const current = connection.current;
      connection.current = null;
      setConnectionSource(null);
      if (!current) return;
      const valid = current.lease.isCurrent();
      current.lease.finish();
      const point = clientPoint(event);
      const target = event.target instanceof Element ? event.target : null;
      if (
        !valid ||
        current.submitted ||
        current.intent !== "connect" ||
        !point ||
        event.type.includes("cancel") ||
        state.toHandle ||
        state.toNode ||
        target?.closest(".react-flow__node, .react-flow__handle") ||
        Math.hypot(point.x - current.start.x, point.y - current.start.y) < 5
      )
        return;
      const pin = modelRef.current.pins[current.sourceId];
      if (!pin) return;
      interaction.setContextMenu({ x: point.x, y: point.y, visible: true });
      interaction.setPendingConnection(pin.address);
    },
    [interaction.setContextMenu, interaction.setPendingConnection],
  );

  // React Flow installs these callbacks in its store and memoized element renderers.
  // Changing their identity on every pointer frame broadcasts to all nodes and handles.
  const onMoveStart = useCallback<OnMove>(
    (event) => {
      // Session synchronization emits { sync: true }, not a user input event.
      if (
        !event ||
        typeof event.type !== "string" ||
        isDragging() ||
        connection.current?.lease.isCurrent() ||
        selectionGesture.current?.isCurrent()
      )
        return;
      pan.current?.finish();
      const start = viewport.getViewport();
      pan.current = interaction.beginGesture("panning", () => {
        suppressClick.current = true;
        flowStore.setState({ paneDragging: false });
        viewport.setViewport(start);
        synchronizeViewport(start);
      });
    },
    [interaction.beginGesture, flowStore, viewport, synchronizeViewport, isDragging],
  );
  const onMoveEnd = useCallback<OnMove>(
    (event) => {
      if (!event || typeof event.type !== "string") return;
      const current = pan.current;
      pan.current = null;
      if (!current) return;
      const valid = current.isCurrent();
      current.finish();
      if (valid) viewport.commit();
      else {
        // D3 still receives mouse moves until release; restore its private transform too.
        synchronizeViewport(viewport.getViewport());
      }
    },
    [viewport, synchronizeViewport],
  );
  const onMove = useCallback<OnMove>(
    (event, next) => {
      if (!event || typeof event.type !== "string") return;
      // React Flow has already applied its transform. Publish coordinates without a React
      // render/effect round trip; the session subscriber observes an unchanged transform.
      if (pan.current?.isCurrent())
        viewport.setViewport({ x: next.x, y: next.y, scale: next.zoom });
      else synchronizeViewport(viewport.getViewport());
    },
    [viewport, synchronizeViewport],
  );
  const onNodeClick = useCallback<NodeMouseHandler<FlowNode>>(
    (event, node) => {
      if (
        event.shiftKey ||
        event.ctrlKey ||
        event.metaKey ||
        suppressClick.current ||
        !interaction.isInteractive()
      )
        return;
      void commands.revealNodeDetails(node.id);
    },
    [commands.revealNodeDetails, interaction.isInteractive],
  );
  const onSelectionStart = useCallback(() => {
    const snapshot = selectionPointer.current ?? {
      nodeIds: [...workspace.selectedNodeIds],
      connectionIds: [...workspace.selectedConnectionIds],
      shiftKey: false,
    };
    selectionPointer.current = snapshot;
    selectionGesture.current = interaction.beginGesture("selecting", () => {
      suppressClick.current = true;
      resetSelectionOverlay();
      if (snapshot.connectionIds.length)
        commands.setSelectedConnectionIds(snapshot.connectionIds, groupId);
      else commands.setSelectedNodeIds(snapshot.nodeIds, groupId);
    });
  }, [
    workspace.selectedNodeIds,
    workspace.selectedConnectionIds,
    interaction.beginGesture,
    resetSelectionOverlay,
    commands.setSelectedConnectionIds,
    commands.setSelectedNodeIds,
    groupId,
  ]);
  const onPaneClick = useCallback(
    (event: ReactMouseEvent) => {
      if (!event.shiftKey && !suppressClick.current && interaction.isInteractive())
        commands.setSelectedNodeIds([], groupId);
    },
    [commands.setSelectedNodeIds, interaction.isInteractive, groupId],
  );
  const onEdgeClick = useCallback<EdgeMouseHandler<FlowEdge>>(
    (event, edge) => {
      if (!interaction.isInteractive() || event.detail > 1) return;
      const before = {
        nodeIds: new Set(workspace.selectedNodeIds),
        connectionIds: new Set(workspace.selectedConnectionIds),
      };
      const toggle = event.ctrlKey || event.metaKey || event.shiftKey;
      const ids = toggle ? before.connectionIds : new Set<string>();
      if (toggle && ids.has(edge.id)) ids.delete(edge.id);
      else ids.add(edge.id);
      const temporary = { nodeIds: new Set<string>(), connectionIds: new Set(ids) };
      beforeEdgeClick.current = {
        id: edge.id,
        before: {
          nodeIds: new Set(workspace.selectedNodeIds),
          connectionIds: new Set(workspace.selectedConnectionIds),
        },
        temporary,
      };
      commands.setSelectedConnectionIds([...ids], groupId);
      setEdgeMenu(null);
    },
    [
      interaction.isInteractive,
      workspace.selectedNodeIds,
      workspace.selectedConnectionIds,
      commands.setSelectedConnectionIds,
      groupId,
    ],
  );
  const onEdgeDoubleClick = useCallback<EdgeMouseHandler<FlowEdge>>(
    (event, edge) => {
      if (!interaction.isInteractive()) return;
      const element = flowStore.getState().domNode;
      if (!element) return;
      const rect = element.getBoundingClientRect();
      const currentViewport = viewport.getViewport();
      const position = {
        x: (event.clientX - rect.left - currentViewport.x) / currentViewport.scale,
        y: (event.clientY - rect.top - currentViewport.y) / currentViewport.scale,
      };
      const selected = {
        nodeIds: new Set(workspace.selectedNodeIds),
        connectionIds: new Set(workspace.selectedConnectionIds),
      };
      const pending = beforeEdgeClick.current;
      const snapshot =
        pending?.id === edge.id ? pending : { before: selected, temporary: selected };
      beforeEdgeClick.current = null;
      void interaction.insertRerouteAtConnection(edge.id, position, graphPath, groupId, snapshot);
    },
    [
      interaction.isInteractive,
      interaction.insertRerouteAtConnection,
      flowStore,
      viewport,
      workspace.selectedNodeIds,
      workspace.selectedConnectionIds,
      graphPath,
      groupId,
    ],
  );
  const onEdgeContextMenu = useCallback<EdgeMouseHandler<FlowEdge>>(
    (event, edge) => {
      event.preventDefault();
      if (!interaction.isInteractive()) return;
      const ids = workspace.selectedConnectionIds.includes(edge.id)
        ? workspace.selectedConnectionIds
        : [edge.id];
      commands.setSelectedConnectionIds(ids, groupId);
      setEdgeMenu({ x: event.clientX, y: event.clientY, ids });
    },
    [
      interaction.isInteractive,
      workspace.selectedConnectionIds,
      commands.setSelectedConnectionIds,
      groupId,
    ],
  );

  return (
    <GraphFlowContext.Provider value={context}>
      <GraphFlowInteractionContext.Provider value={connectionStore}>
        <GraphFlowFeedback store={connectionStore} feedback={feedback} />
        <ReactFlow<FlowNode, FlowEdge>
          id={panelInstanceId}
          className="yss-flow"
          data-connection-active={feedback.sourceId !== null || undefined}
          edges={edges}
          nodeTypes={nodeTypes}
          edgeTypes={edgeTypes}
          // Capture the grab point on press; threshold activation discards the first movement.
          nodeDragThreshold={0}
          defaultViewport={defaultViewport}
          minZoom={EDITOR_VIEWPORT_SCALE_LIMITS.min}
          maxZoom={EDITOR_VIEWPORT_SCALE_LIMITS.max}
          onMoveStart={onMoveStart}
          onMoveEnd={onMoveEnd}
          onNodesChange={onNodesChange}
          onNodeClick={onNodeClick}
          onNodeDragStart={startNodeDrag}
          onNodeDragStop={stopNodeDrag}
          onSelectionStart={onSelectionStart}
          onPointerUp={() => {
            // React Flow enables its group overlay after onSelectionEnd, before pointerup bubbles here.
            resetSelectionOverlay();
            selectionGesture.current?.finish();
            selectionGesture.current = null;
            selectionPointer.current = null;
          }}
          onPointerCancel={() => {
            if (contextMenuPress.current) contextMenuPress.current.moved = true;
            const current = selectionGesture.current;
            const snapshot = current?.isCurrent() ? selectionPointer.current : null;
            current?.finish();
            resetSelectionOverlay();
            selectionGesture.current = null;
            selectionPointer.current = null;
            if (snapshot) {
              suppressClick.current = true;
              if (snapshot.connectionIds.length)
                commands.setSelectedConnectionIds(snapshot.connectionIds, groupId);
              else commands.setSelectedNodeIds(snapshot.nodeIds, groupId);
            }
          }}
          onPaneClick={onPaneClick}
          onContextMenuCapture={(event) => {
            const target = event.target instanceof Element ? event.target : null;
            if (!target?.classList.contains("react-flow__pane")) return;
            // Linux may dispatch contextmenu on press, before a pan can be detected.
            // Pointer right-clicks open on release; consume native events in either order.
            event.preventDefault();
            event.stopPropagation();
            if (
              !interactive ||
              !interaction.isInteractive() ||
              (event.button === 2 && contextMenuPress.current !== null) ||
              (pan.current && !pan.current.isCurrent())
            )
              return;
            onContextMenu(event);
          }}
          onMove={onMove}
          onEdgeClick={onEdgeClick}
          onEdgeDoubleClick={onEdgeDoubleClick}
          onEdgeContextMenu={onEdgeContextMenu}
          onPointerDownCapture={(event) => {
            const target = event.target instanceof Element ? event.target : null;
            contextMenuPress.current =
              event.button === 2 && target?.classList.contains("react-flow__pane")
                ? { x: event.clientX, y: event.clientY, moved: false, released: false }
                : null;
            suppressClick.current = false;
            // A new press also recovers an aborted gesture whose release happened outside the view.
            if (selectionGesture.current && !selectionGesture.current.isCurrent()) {
              selectionGesture.current.finish();
              selectionGesture.current = null;
            }
            recoverCancelledDrag();
            if (pan.current && !pan.current.isCurrent()) {
              pan.current.finish();
              pan.current = null;
              synchronizeViewport(viewport.getViewport());
            }
            // Snapshot before React Flow clears its selection on the first rectangle movement.
            selectionPointer.current =
              event.button === 0 && target?.classList.contains("react-flow__pane")
                ? {
                    nodeIds: [...workspace.selectedNodeIds],
                    connectionIds: [...workspace.selectedConnectionIds],
                    shiftKey: event.shiftKey,
                  }
                : null;
            const handle = target?.closest<HTMLElement>(".react-flow__handle");
            const pin = handle?.dataset.handleid
              ? modelRef.current.pins[handle.dataset.handleid]
              : null;
            if (!pin) return;
            const action = resolveFlowPinAction(event, pin);
            if (!interaction.isInteractive() || action === "none" || action === "disconnect") {
              event.preventDefault();
              event.stopPropagation();
              if (interaction.isInteractive() && action === "disconnect") {
                void interaction.mutations
                  .disconnectPort(graphPath, pin.id)
                  .then((outcome) => {
                    if (outcome.status === "failed")
                      interaction.mutations.reportMutationFailure({
                        graphPath,
                        intent: "disconnectPort",
                      });
                  })
                  .catch(() =>
                    interaction.mutations.reportMutationFailure({
                      graphPath,
                      intent: "disconnectPort",
                    }),
                  );
              }
            }
          }}
          onPointerMoveCapture={(event) => {
            const press = contextMenuPress.current;
            if (press && !press.released && (event.buttons & 2) !== 0)
              press.moved ||= Math.hypot(event.clientX - press.x, event.clientY - press.y) > 3;
          }}
          onPointerUpCapture={(event) => {
            const target = event.target instanceof Element ? event.target : null;
            const press = contextMenuPress.current;
            if (event.button === 2 && press && !press.released) {
              press.released = true;
              if (
                interactive &&
                interaction.isInteractive() &&
                target?.classList.contains("react-flow__pane") &&
                !press.moved &&
                Math.hypot(event.clientX - press.x, event.clientY - press.y) <= 3 &&
                (!pan.current || pan.current.isCurrent())
              )
                onContextMenu(event);
              return;
            }
            const current = selectionGesture.current;
            const snapshot = selectionPointer.current;
            const cancelled = current !== null && !current.isCurrent();
            const additiveClick = !current && snapshot?.shiftKey;
            if (
              event.button !== 0 ||
              !target?.classList.contains("react-flow__pane") ||
              (!cancelled && !additiveClick)
            )
              return;
            resetSelectionOverlay();
            current?.finish();
            selectionGesture.current = null;
            selectionPointer.current = null;
            suppressClick.current = true;
            if (target.hasPointerCapture?.(event.pointerId))
              target.releasePointerCapture(event.pointerId);
            // Pane handles a zero-sized rectangle as a click during pointerup, before onClick.
            event.stopPropagation();
          }}
          onClickCapture={(event) => {
            if (!suppressClick.current) return;
            suppressClick.current = false;
            event.stopPropagation();
          }}
          onConnectStart={onConnectStart}
          onConnect={onConnect}
          onConnectEnd={onConnectEnd}
          isValidConnection={isValidConnection}
          connectionMode={ConnectionMode.Loose}
          connectionLineComponent={GraphFlowConnection}
          connectionRadius={18}
          connectOnClick={false}
          {...flowCanvasInteractionProps(interactive)}
          nodesConnectable={interactive}
          edgesReconnectable={false}
          proOptions={{ hideAttribution: true }}
        >
          {pendingSource && interaction.contextMenu ? (
            <PendingFlowConnection pin={pendingSource} menu={interaction.contextMenu} />
          ) : null}
        </ReactFlow>
        {interactive && edgeMenu ? (
          <ConnectionContextMenu
            position={edgeMenu}
            selectedCount={edgeMenu.ids.length}
            onBreak={() => {
              void commands.breakConnectionsById(edgeMenu.ids, graphPath, groupId);
            }}
            onClose={() => setEdgeMenu(null)}
          />
        ) : null}
      </GraphFlowInteractionContext.Provider>
    </GraphFlowContext.Provider>
  );
}
