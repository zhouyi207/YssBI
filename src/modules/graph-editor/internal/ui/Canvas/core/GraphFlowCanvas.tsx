import {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
  type MouseEvent as ReactMouseEvent,
} from "react";
import {
  ReactFlow,
  ReactFlowProvider,
  ConnectionMode,
  SelectionMode,
  useStoreApi,
  type Connection,
  type NodeChange,
  type OnConnectStart,
  type OnConnectEnd,
  type Viewport,
} from "@xyflow/react";
import type { EditorCanvasSession, GraphContextMenuActions } from "@/features/application/editor";
import { useGraphRead } from "@/features/core/graph/read";
import {
  EDITOR_VIEWPORT_SCALE_LIMITS,
  type EditorViewport,
} from "@/features/core/viewport/editorViewport";
import { ConnectionContextMenu } from "../../ContextMenu";
import { GraphFlowContext } from "./GraphFlowContext";
import { GraphFlowNode } from "./GraphFlowNode";
import { GraphFlowEdge } from "./GraphFlowEdge";
import { GraphFlowConnection, PendingFlowConnection } from "./GraphFlowConnection";
import {
  buildGraphFlowModel,
  resolveFlowConnection,
  resolveFlowPinAction,
  type GraphFlowNode as FlowNode,
  type GraphFlowEdge as FlowEdge,
  type FlowConnectionIntent,
} from "./graphFlowModel";
import "@xyflow/react/dist/base.css";
import "./graphFlow.css";

const nodeTypes = { graph: GraphFlowNode };
const edgeTypes = { graph: GraphFlowEdge };
const multiSelectionKeys = ["Shift", "Control", "Meta"];
const panButtons = [1, 2];
type GestureLease = NonNullable<ReturnType<EditorCanvasSession["interaction"]["beginGesture"]>>;
type PositionPreview = { position: { x: number; y: number }; dragging: boolean; owner: object };
type NodeMeasurement = { width: number; height: number };
type SelectionPointerSnapshot = {
  nodeIds: string[];
  connectionIds: string[];
  nodesSelectionActive: boolean;
  shiftKey: boolean;
};

interface GraphFlowCanvasProps {
  panelInstanceId: string;
  graphPath: string;
  groupId: string;
  interactive: boolean;
  contextMenuActions: GraphContextMenuActions | null;
  canvas: EditorCanvasSession;
  viewport: EditorViewport;
  onViewportChange(viewport: EditorViewport): void;
  onViewportCommit(): void;
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

function GraphFlowRuntime({
  graphPath,
  groupId,
  panelInstanceId,
  interactive,
  canvas,
  contextMenuActions,
  viewport,
  onViewportChange,
  onViewportCommit,
  onContextMenu,
}: GraphFlowCanvasProps) {
  const flowStore = useStoreApi<FlowNode, FlowEdge>();
  const viewportRef = useRef(viewport);
  viewportRef.current = viewport;
  const bucket = useGraphRead((snapshot) => snapshot.graphEntities[graphPath]);
  const model = useMemo(() => buildGraphFlowModel(bucket), [bucket]);
  const modelRef = useRef(model);
  modelRef.current = model;
  const { interaction, commands, workspace } = canvas;
  const [positions, setPositions] = useState<Record<string, PositionPreview>>({});
  const [measurements, setMeasurements] = useState<Record<string, NodeMeasurement>>({});
  const renderedNodes = useRef(new Map<string, { projection: FlowNode; view: FlowNode }>());
  const positionsRef = useRef(positions);
  const drag = useRef<{ lease: GestureLease; owner: object } | null>(null);
  const pan = useRef<GestureLease | null>(null);
  const selectionGesture = useRef<GestureLease | null>(null);
  const selectionPointer = useRef<SelectionPointerSnapshot | null>(null);
  const suppressClick = useRef(false);
  const connection = useRef<{
    lease: GestureLease;
    sourceId: string;
    intent: FlowConnectionIntent;
    start: { x: number; y: number };
    submitted: boolean;
  } | null>(null);
  const [connectionSource, setConnectionSource] = useState<{
    sourceId: string;
    intent: FlowConnectionIntent;
  } | null>(null);
  const [edgeMenu, setEdgeMenu] = useState<{ x: number; y: number; ids: string[] } | null>(null);
  const beforeEdgeClick = useRef<{
    id: string;
    before: { nodeIds: Set<string>; connectionIds: Set<string> };
    temporary: { nodeIds: Set<string>; connectionIds: Set<string> };
  } | null>(null);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      drag.current?.lease.finish();
      connection.current?.lease.finish();
      pan.current?.finish();
      selectionGesture.current?.finish();
    };
  }, []);
  useEffect(() => {
    if (!interactive) setEdgeMenu(null);
  }, [interactive]);
  useEffect(() => {
    setMeasurements((current) =>
      Object.keys(current).some((id) => !model.nodeIds.has(id))
        ? Object.fromEntries(Object.entries(current).filter(([id]) => model.nodeIds.has(id)))
        : current,
    );
  }, [model.nodeIds]);

  const clearPositions = useCallback((owner: object) => {
    if (!mounted.current) return;
    const next = Object.fromEntries(
      Object.entries(positionsRef.current).filter(([, entry]) => entry.owner !== owner),
    );
    positionsRef.current = next;
    setPositions(next);
  }, []);
  const nodes = useMemo(() => {
    const selected = new Set(workspace.selectedNodeIds);
    const nextCache = new Map<string, { projection: FlowNode; view: FlowNode }>();
    const next = model.nodes.map((node) => {
      const position = positions[node.id]?.position ?? node.position;
      const dragging = positions[node.id]?.dragging ?? false;
      const measured = measurements[node.id];
      const isSelected = interactive && selected.has(node.id);
      const selectable = interactive && node.selectable;
      const draggable = interactive && node.draggable;
      const previous = renderedNodes.current.get(node.id);
      const view =
        previous?.projection === node &&
        previous.view.position.x === position.x &&
        previous.view.position.y === position.y &&
        previous.view.measured === measured &&
        previous.view.dragging === dragging &&
        previous.view.selected === isSelected &&
        previous.view.selectable === selectable &&
        previous.view.draggable === draggable
          ? previous.view
          : { ...node, position, measured, dragging, selected: isSelected, selectable, draggable };
      nextCache.set(node.id, { projection: node, view });
      return view;
    });
    renderedNodes.current = nextCache;
    return next;
  }, [model.nodes, positions, measurements, workspace.selectedNodeIds, interactive]);
  const edges = useMemo(() => {
    const selected = new Set(workspace.selectedConnectionIds);
    return model.edges.map((edge) => ({ ...edge, selected: interactive && selected.has(edge.id) }));
  }, [model.edges, workspace.selectedConnectionIds, interactive]);
  const feedbackForPin = useCallback(
    (pinId: string) => {
      const sourceId = connectionSource?.sourceId ?? interaction.pendingConnection?.id;
      return sourceId
        ? resolveFlowConnection(model, sourceId, pinId, connectionSource?.intent ?? "connect")
        : null;
    },
    [model, connectionSource, interaction.pendingConnection],
  );
  const sourcePin =
    (connectionSource ? model.pins[connectionSource.sourceId] : null) ??
    interaction.pendingConnection;
  const context = useMemo(
    () => ({
      graphPath,
      groupId,
      interactive,
      contextMenuActions,
      model,
      sourcePin,
      feedbackForPin,
    }),
    [graphPath, groupId, interactive, contextMenuActions, model, sourcePin, feedbackForPin],
  );
  const flowViewport = useMemo(
    () => ({ x: viewport.x, y: viewport.y, zoom: viewport.scale }),
    [viewport],
  );

  const startNodeDrag = useCallback(() => {
    if (drag.current?.lease.isCurrent()) return;
    const owner = {};
    const lease = interaction.beginGesture("draggingNodes", () => {
      suppressClick.current = true;
      clearPositions(owner);
    });
    drag.current = lease ? { lease, owner } : null;
  }, [interaction.beginGesture, clearPositions]);

  const resetSelectionOverlay = useCallback(
    (snapshot: SelectionPointerSnapshot | null) => {
      flowStore.setState({
        userSelectionActive: false,
        userSelectionRect: null,
        nodesSelectionActive: snapshot?.nodesSelectionActive ?? false,
      });
    },
    [flowStore],
  );
  const stopNodeDrag = useCallback(() => {
    const current = drag.current;
    drag.current = null;
    if (!current) return;
    const submitted = Object.entries(positionsRef.current).flatMap(([nodeId, entry]) =>
      entry.owner === current.owner ? [{ nodeId, position: entry.position }] : [],
    );
    const valid = current.lease.isCurrent();
    current.lease.finish();
    if (!valid || !submitted.length) {
      clearPositions(current.owner);
      return;
    }
    const settled = Object.fromEntries(
      Object.entries(positionsRef.current).map(([id, entry]) => [
        id,
        entry.owner === current.owner ? { ...entry, dragging: false } : entry,
      ]),
    );
    positionsRef.current = settled;
    setPositions(settled);
    void interaction.mutations
      .submitNodePositions(graphPath, submitted)
      .then((outcome) => {
        if (outcome.status === "failed")
          interaction.mutations.reportMutationFailure({
            graphPath,
            intent: "moveNodes",
            message: outcome.message,
          });
      })
      .catch(() => interaction.mutations.reportMutationFailure({ graphPath, intent: "moveNodes" }))
      .finally(() => clearPositions(current.owner));
  }, [graphPath, interaction.mutations, clearPositions]);
  const onNodesChange = useCallback(
    (changes: NodeChange<FlowNode>[]) => {
      // Controlled nodes must retain renderer measurements. Dropping them resets handle bounds
      // and makes React Flow hide the node while measuring it again on every pointer frame.
      const dimensions = changes.filter((change) => change.type === "dimensions");
      if (dimensions.length)
        setMeasurements((current) => {
          let next = current;
          for (const change of dimensions) {
            const size = change.dimensions;
            if (
              !modelRef.current.nodeIds.has(change.id) ||
              !size ||
              !Number.isFinite(size.width) ||
              !Number.isFinite(size.height) ||
              size.width <= 0 ||
              size.height <= 0
            )
              continue;
            if (
              current[change.id]?.width === size.width &&
              current[change.id]?.height === size.height
            )
              continue;
            if (next === current) next = { ...current };
            next[change.id] = { width: size.width, height: size.height };
          }
          return next;
        });
      if (!interaction.isInteractive()) return;
      const selectionChanges = changes.filter((change) => change.type === "select");
      if (
        selectionChanges.length &&
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
      const current = drag.current;
      if (!current?.lease.isCurrent()) return;
      const next = { ...positionsRef.current };
      let changed = false;
      for (const change of changes) {
        if (
          change.type !== "position" ||
          !change.position ||
          !modelRef.current.draggableNodeIds.has(change.id)
        )
          continue;
        if (!Number.isFinite(change.position.x) || !Number.isFinite(change.position.y)) continue;
        next[change.id] = {
          position: { ...change.position },
          dragging: change.dragging ?? true,
          owner: current.owner,
        };
        changed = true;
      }
      if (changed) {
        positionsRef.current = next;
        setPositions(next);
      }
    },
    [
      commands.setSelectedNodeIds,
      groupId,
      interaction.isInteractive,
      workspace.selectedNodeIds.length,
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
        resolveFlowConnection(
          modelRef.current,
          target.current.sourceId,
          target.targetId,
          target.current.intent,
        ).kind !== "invalid"
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
          if (outcome.status === "failed" && outcome.message)
            interaction.mutations.reportMutationFailure({
              graphPath,
              intent: target.current.intent,
              message: outcome.message,
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
      interaction.setPendingConnection(pin);
    },
    [interaction.setContextMenu, interaction.setPendingConnection],
  );

  return (
    <GraphFlowContext.Provider value={context}>
      <ReactFlow<FlowNode, FlowEdge>
        id={panelInstanceId}
        className="yss-flow"
        nodes={nodes}
        edges={edges}
        nodeTypes={nodeTypes}
        edgeTypes={edgeTypes}
        viewport={flowViewport}
        onViewportChange={(next: Viewport) => {
          if (!interaction.isInteractive() || !pan.current?.isCurrent()) return;
          const nextViewport = { x: next.x, y: next.y, scale: next.zoom };
          viewportRef.current = nextViewport;
          onViewportChange(nextViewport);
        }}
        minZoom={EDITOR_VIEWPORT_SCALE_LIMITS.min}
        maxZoom={EDITOR_VIEWPORT_SCALE_LIMITS.max}
        onMoveStart={(event) => {
          // Controlled viewport synchronization emits { sync: true }, not a user input event.
          if (
            !event ||
            typeof event.type !== "string" ||
            drag.current?.lease.isCurrent() ||
            connection.current?.lease.isCurrent() ||
            selectionGesture.current?.isCurrent()
          )
            return;
          pan.current?.finish();
          const start = viewportRef.current;
          pan.current = interaction.beginGesture("panning", () => {
            suppressClick.current = true;
            flowStore.setState({ paneDragging: false });
            viewportRef.current = start;
            onViewportChange(start);
            flowStore
              .getState()
              .panZoom?.syncViewport({ x: start.x, y: start.y, zoom: start.scale });
          });
        }}
        onMoveEnd={(event) => {
          if (!event || typeof event.type !== "string") return;
          const current = pan.current;
          pan.current = null;
          if (!current) return;
          const valid = current.isCurrent();
          current.finish();
          if (valid) onViewportCommit();
          else {
            // D3 still receives mouse moves until release; restore its private transform too.
            const restored = viewportRef.current;
            flowStore
              .getState()
              .panZoom?.syncViewport({ x: restored.x, y: restored.y, zoom: restored.scale });
          }
        }}
        onNodesChange={onNodesChange}
        onNodeDragStart={startNodeDrag}
        onNodeDragStop={stopNodeDrag}
        onSelectionDragStart={startNodeDrag}
        onSelectionDragStop={stopNodeDrag}
        onSelectionStart={() => {
          const snapshot = selectionPointer.current ?? {
            nodeIds: [...workspace.selectedNodeIds],
            connectionIds: [...workspace.selectedConnectionIds],
            nodesSelectionActive: false,
            shiftKey: false,
          };
          selectionPointer.current = snapshot;
          selectionGesture.current = interaction.beginGesture("selecting", () => {
            suppressClick.current = true;
            resetSelectionOverlay(snapshot);
            if (snapshot.connectionIds.length)
              commands.setSelectedConnectionIds(snapshot.connectionIds, groupId);
            else commands.setSelectedNodeIds(snapshot.nodeIds, groupId);
          });
        }}
        onSelectionEnd={() => {
          selectionGesture.current?.finish();
          selectionGesture.current = null;
          selectionPointer.current = null;
        }}
        onPaneClick={(event) => {
          if (!event.shiftKey && !suppressClick.current && interaction.isInteractive())
            commands.setSelectedNodeIds([], groupId);
        }}
        onPaneContextMenu={
          interactive
            ? (event) => {
                if (pan.current && !pan.current.isCurrent()) {
                  event.preventDefault();
                  return;
                }
                onContextMenu(event);
              }
            : undefined
        }
        onEdgeClick={(event, edge) => {
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
        }}
        onEdgeDoubleClick={(event, edge) => {
          if (!interaction.isInteractive()) return;
          const element = flowStore.getState().domNode;
          if (!element) return;
          const rect = element.getBoundingClientRect();
          const position = {
            x: (event.clientX - rect.left - viewport.x) / viewport.scale,
            y: (event.clientY - rect.top - viewport.y) / viewport.scale,
          };
          const selected = {
            nodeIds: new Set(workspace.selectedNodeIds),
            connectionIds: new Set(workspace.selectedConnectionIds),
          };
          const pending = beforeEdgeClick.current;
          const snapshot =
            pending?.id === edge.id ? pending : { before: selected, temporary: selected };
          beforeEdgeClick.current = null;
          void interaction.insertRerouteAtConnection(
            edge.id,
            position,
            graphPath,
            groupId,
            snapshot,
          );
        }}
        onEdgeContextMenu={(event, edge) => {
          event.preventDefault();
          if (!interaction.isInteractive()) return;
          const ids = workspace.selectedConnectionIds.includes(edge.id)
            ? workspace.selectedConnectionIds
            : [edge.id];
          commands.setSelectedConnectionIds(ids, groupId);
          setEdgeMenu({ x: event.clientX, y: event.clientY, ids });
        }}
        onPointerDownCapture={(event) => {
          suppressClick.current = false;
          // A new press also recovers an aborted gesture whose release happened outside the view.
          if (selectionGesture.current && !selectionGesture.current.isCurrent()) {
            selectionGesture.current.finish();
            selectionGesture.current = null;
          }
          if (drag.current && !drag.current.lease.isCurrent()) {
            drag.current.lease.finish();
            drag.current = null;
          }
          if (pan.current && !pan.current.isCurrent()) {
            pan.current.finish();
            pan.current = null;
            const current = viewportRef.current;
            flowStore
              .getState()
              .panZoom?.syncViewport({ x: current.x, y: current.y, zoom: current.scale });
          }
          const target = event.target instanceof Element ? event.target : null;
          // Snapshot before React Flow clears its selection on the first rectangle movement.
          selectionPointer.current =
            event.button === 0 && target?.classList.contains("react-flow__pane")
              ? {
                  nodeIds: [...workspace.selectedNodeIds],
                  connectionIds: [...workspace.selectedConnectionIds],
                  nodesSelectionActive: flowStore.getState().nodesSelectionActive,
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
                      message: outcome.message,
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
        onPointerUpCapture={(event) => {
          const target = event.target instanceof Element ? event.target : null;
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
          resetSelectionOverlay(snapshot);
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
        nodesDraggable={interactive}
        nodesConnectable={interactive}
        elementsSelectable={interactive}
        edgesReconnectable={false}
        deleteKeyCode={null}
        disableKeyboardA11y
        selectionOnDrag={interactive}
        selectionMode={SelectionMode.Partial}
        selectionKeyCode={null}
        multiSelectionKeyCode={multiSelectionKeys}
        panOnDrag={interactive ? panButtons : false}
        panActivationKeyCode={interactive ? "Alt" : null}
        zoomOnScroll={interactive}
        zoomOnPinch={interactive}
        zoomOnDoubleClick={false}
        autoPanOnNodeDrag={false}
        autoPanOnConnect={false}
        autoPanOnSelection={false}
        proOptions={{ hideAttribution: true }}
      >
        {interaction.pendingConnection && interaction.contextMenu ? (
          <PendingFlowConnection
            pin={interaction.pendingConnection}
            menu={interaction.contextMenu}
          />
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
    </GraphFlowContext.Provider>
  );
}
