import { useEffect, useMemo, useRef, useState } from "react";
import {
  Background,
  Handle,
  Position,
  ReactFlow,
  ReactFlowProvider,
  getNodesBounds,
  useReactFlow,
  useStoreApi,
  type NodeChange,
  type NodeProps,
  type Viewport,
} from "@xyflow/react";
import "@xyflow/react/dist/style.css";
import {
  useEditorPaneStateStore,
  EMPTY_EDITOR_PANE_SELECTION,
  workbenchLayoutRead,
  type EditorPanelScope,
} from "@/modules/workbench/public";
import type { MindSnapshot } from "@/shared/types/domain/mind";
import { useMindProjectionStore } from "@/features/core/resource/mindProjectionStore";
import { mindActions, deleteMindNodes } from "@/features/application/resource/mindActions";
import {
  revealDetails,
  setInspectionContext,
  detailFocusForEditorResource,
} from "@/features/application/editor/rightSidebarActions";
import {
  captureEditorCommandTarget,
  captureEditorShortcutTarget,
  isEditorCommandTargetCurrent,
  shouldIgnoreEditorShortcutEvent,
  type EditorCommandTarget,
} from "@/features/application/editor/editorCommandFocus";
import { resolveCanvasShortcut } from "@/features/core/keyboard/canvasShortcut";
import { EDITOR_VIEWPORT_SCALE_LIMITS } from "@/features/core/viewport/editorViewport";
import { fitWorldBounds } from "@/features/core/viewport/fitViewport";
import { flowCanvasInteractionProps } from "@/shared/ui/flowCanvasInteraction";
import { projectMindMap } from "./mindProjection";
import { FileEditor } from "./FileEditor";

function MindNodeView({ data, selected }: NodeProps) {
  return (
    <div
      className={`w-52 rounded-lg border bg-card px-4 py-3 text-sm shadow-sm ${selected ? "border-primary ring-1 ring-primary" : "border-border"}`}
    >
      <Handle type="target" position={Position.Left} className="opacity-0" isConnectable={false} />
      <div className="line-clamp-3 whitespace-pre-wrap break-words">{String(data.label)}</div>
      {data.collapsed === true && <span className="text-xs text-muted-foreground">…</span>}
      <Handle type="source" position={Position.Right} className="opacity-0" isConnectable={false} />
    </div>
  );
}
const nodeTypes = { mind: MindNodeView };

type Gesture = { target: EditorCommandTarget; cancelled: boolean };
type SelectionSnapshot = { nodeIds: string[]; shiftKey: boolean };
const edgeOptions = { selectable: false, focusable: false };

function MindEditor({
  snapshot,
  panelInstanceId,
  isVisible,
  reportError,
}: {
  snapshot: MindSnapshot;
  panelInstanceId: string;
  isVisible: boolean;
  reportError(error: unknown): void;
}) {
  const mind = snapshot.content;
  const flow = useReactFlow();
  const flowStore = useStoreApi();
  const canvasElement = useRef<HTMLDivElement>(null);
  const mounted = useRef(true);
  const paneSelection = useEditorPaneStateStore((state) => state.selections[panelInstanceId]);
  const selectedIds = useMemo(
    () => paneSelection?.selectedNodeIds ?? EMPTY_EDITOR_PANE_SELECTION.selectedNodeIds,
    [paneSelection],
  );
  const collapsedIds = useEditorPaneStateStore((state) => state.collapsedNodeIds[panelInstanceId]);
  const collapsed = useMemo(() => new Set(collapsedIds), [collapsedIds]);
  const projection = useMemo(() => projectMindMap(mind, collapsed), [mind, collapsed]);
  const [measurements, setMeasurements] = useState<
    Record<string, { width: number; height: number }>
  >({});
  const [viewport, setViewport] = useState<Viewport>({ x: 0, y: 0, zoom: 1 });
  const viewportRef = useRef(viewport);
  const pan = useRef<(Gesture & { start: Viewport }) | null>(null);
  const selection = useRef<(Gesture & { before: SelectionSnapshot }) | null>(null);
  const selectionPointer = useRef<SelectionSnapshot | null>(null);
  const suppressClick = useRef(false);
  const cancelRef = useRef<() => boolean>(() => false);
  const nodes = useMemo(() => {
    const selected = new Set(selectedIds);
    return projection.nodes.map((item) => ({
      ...item,
      selected: selected.has(item.id),
      measured: measurements[item.id],
    }));
  }, [projection.nodes, measurements, selectedIds]);

  const readSelection = () =>
    useEditorPaneStateStore.getState().selections[panelInstanceId]?.selectedNodeIds ??
    EMPTY_EDITOR_PANE_SELECTION.selectedNodeIds;
  const captureTarget = () => {
    if (!isVisible) return null;
    const target = captureEditorCommandTarget(panelInstanceId);
    return target?.resourceKind === "mind" &&
      target.resourceRef === snapshot.path &&
      isEditorCommandTargetCurrent(target)
      ? target
      : null;
  };
  const setSelection = (ids: string[]) => {
    const current = readSelection();
    if (current.length !== ids.length || current.some((id, index) => id !== ids[index]))
      useEditorPaneStateStore.getState().setSelectedNodeIds(panelInstanceId, ids);
    setInspectionContext(
      { resourceKind: "mind", resourceRef: snapshot.path, panelInstanceId },
      ids,
    );
  };
  const run = (operation: () => Promise<unknown>) => {
    void operation().catch((error) => {
      if (mounted.current) reportError(error);
    });
  };
  const restoreViewport = (next: Viewport) => {
    viewportRef.current = next;
    setViewport(next);
    flowStore.getState().panZoom?.syncViewport(next);
  };
  const resetSelectionOverlay = () => {
    flowStore.setState({
      userSelectionActive: false,
      userSelectionRect: null,
      nodesSelectionActive: false,
    });
  };
  const cancelGesture = () => {
    let cancelled = false;
    if (pan.current && !pan.current.cancelled) {
      pan.current.cancelled = true;
      flowStore.setState({ paneDragging: false });
      restoreViewport(pan.current.start);
      cancelled = true;
    }
    if (selection.current && !selection.current.cancelled) {
      selection.current.cancelled = true;
      const before = selection.current.before;
      resetSelectionOverlay();
      const panel = workbenchLayoutRead.getPanel(panelInstanceId);
      if (panel?.metadata.role === "editor" && panel.metadata.resourceRef === snapshot.path)
        setSelection(before.nodeIds);
      cancelled = true;
    }
    if (cancelled) suppressClick.current = true;
    return cancelled;
  };
  cancelRef.current = cancelGesture;
  useEffect(() => {
    if (!isVisible) cancelRef.current();
  }, [isVisible]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);

  const onNodesChange = (changes: NodeChange[]) => {
    const dimensions = changes.filter((change) => change.type === "dimensions");
    if (dimensions.length)
      setMeasurements((current) => {
        let next = current;
        for (const change of dimensions) {
          const size = change.dimensions;
          if (
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
    if (!captureTarget()) return;
    const selectionChanges = changes.filter((change) => change.type === "select");
    if (selectionChanges.length && !selection.current?.cancelled) {
      const ids = new Set(readSelection());
      for (const change of selectionChanges) {
        if (change.selected) ids.add(change.id);
        else ids.delete(change.id);
      }
      setSelection([...ids]);
    }
  };
  const fitNodes = (ids?: readonly string[]) => {
    const selected = ids ? new Set(ids) : null;
    const visibleNodes = flow.getNodes().filter((node) => !selected || selected.has(node.id));
    const size = canvasElement.current?.getBoundingClientRect();
    if (!visibleNodes.length || !size?.width || !size.height) return;
    const bounds = getNodesBounds(visibleNodes);
    const next = fitWorldBounds(
      {
        left: bounds.x,
        top: bounds.y,
        right: bounds.x + bounds.width,
        bottom: bounds.y + bounds.height,
      },
      size,
    );
    const view = { x: next.x, y: next.y, zoom: next.scale };
    if (pan.current?.cancelled) pan.current.start = view;
    restoreViewport(view);
  };

  return (
    <div
      ref={canvasElement}
      tabIndex={-1}
      className="relative min-h-0 min-w-0 flex-1"
      onKeyDown={(event) => {
        if (event.defaultPrevented || shouldIgnoreEditorShortcutEvent(event.nativeEvent)) return;
        const target = captureEditorShortcutTarget(event.nativeEvent);
        if (
          target?.panelInstanceId !== panelInstanceId ||
          !isEditorCommandTargetCurrent(target) ||
          !captureTarget()
        )
          return;
        const shortcut = resolveCanvasShortcut(event);
        if (!shortcut) return;
        event.preventDefault();
        event.stopPropagation();
        if (shortcut === "cancel") {
          if (!cancelGesture()) setSelection([]);
        } else if (shortcut === "selectAll") {
          setSelection(projection.nodes.map((node) => node.id));
        } else if (shortcut === "focusSelection") {
          fitNodes(readSelection());
        } else if (shortcut === "fitAll") {
          fitNodes();
        } else {
          cancelGesture();
          const ids = [...readSelection()];
          run(async () => {
            const result = await deleteMindNodes(snapshot, ids);
            if (!result || !isEditorCommandTargetCurrent(target)) return;
            const remaining = new Set(result.content.nodes.map((node) => node.id));
            setSelection(readSelection().filter((id) => remaining.has(id)));
          });
        }
      }}
    >
      <ReactFlow
        id={panelInstanceId}
        {...flowCanvasInteractionProps(isVisible)}
        nodesDraggable={false}
        nodes={nodes}
        edges={projection.edges}
        nodeTypes={nodeTypes}
        defaultEdgeOptions={edgeOptions}
        proOptions={{ hideAttribution: true }}
        fitView
        minZoom={EDITOR_VIEWPORT_SCALE_LIMITS.min}
        maxZoom={EDITOR_VIEWPORT_SCALE_LIMITS.max}
        viewport={viewport}
        nodesConnectable={false}
        edgesReconnectable={false}
        onNodesChange={onNodesChange}
        onNodeClick={(event) => {
          if (
            event.shiftKey ||
            event.ctrlKey ||
            event.metaKey ||
            suppressClick.current ||
            !captureTarget()
          )
            return;
          run(() =>
            revealDetails(detailFocusForEditorResource("mind", snapshot.path, panelInstanceId)),
          );
        }}
        onSelectionStart={() => {
          const target = captureTarget();
          if (!target) return;
          const before = selectionPointer.current ?? {
            nodeIds: [...readSelection()],
            shiftKey: false,
          };
          selection.current = { target, before, cancelled: false };
        }}
        onPointerUp={() => {
          // React Flow enables its group overlay after onSelectionEnd, before pointerup bubbles here.
          resetSelectionOverlay();
          selection.current = null;
          selectionPointer.current = null;
        }}
        onPointerCancel={() => {
          cancelGesture();
          resetSelectionOverlay();
          selection.current = null;
          selectionPointer.current = null;
        }}
        onPaneClick={(event) => {
          if (!event.shiftKey && !suppressClick.current && captureTarget()) setSelection([]);
        }}
        onContextMenuCapture={(event) => event.preventDefault()}
        onPointerDownCapture={(event) => {
          suppressClick.current = false;
          if (pan.current?.cancelled) {
            pan.current = null;
            restoreViewport(viewportRef.current);
          }
          if (selection.current?.cancelled) selection.current = null;
          const target = event.target instanceof Element ? event.target : null;
          selectionPointer.current =
            event.button === 0 && target?.classList.contains("react-flow__pane")
              ? {
                  nodeIds: [...readSelection()],
                  shiftKey: event.shiftKey,
                }
              : null;
        }}
        onPointerUpCapture={(event) => {
          const target = event.target instanceof Element ? event.target : null;
          const before = selectionPointer.current;
          if (
            event.button !== 0 ||
            !target?.classList.contains("react-flow__pane") ||
            (!selection.current?.cancelled && (selection.current || !before?.shiftKey))
          )
            return;
          resetSelectionOverlay();
          selection.current = null;
          selectionPointer.current = null;
          suppressClick.current = true;
          if (target.hasPointerCapture?.(event.pointerId))
            target.releasePointerCapture(event.pointerId);
          event.stopPropagation();
        }}
        onClickCapture={(event) => {
          if (!suppressClick.current) return;
          suppressClick.current = false;
          event.stopPropagation();
        }}
        onMoveStart={(event) => {
          if (
            !event ||
            typeof event.type !== "string" ||
            (selection.current && !selection.current.cancelled)
          )
            return;
          const target = captureTarget();
          if (target) pan.current = { target, start: { ...viewportRef.current }, cancelled: false };
        }}
        onViewportChange={(next) => {
          if (
            !isVisible ||
            pan.current?.cancelled ||
            (pan.current && !isEditorCommandTargetCurrent(pan.current.target))
          )
            return;
          viewportRef.current = next;
          setViewport(next);
        }}
        onMoveEnd={(event) => {
          if (!event || typeof event.type !== "string") return;
          const current = pan.current;
          pan.current = null;
          if (current && (current.cancelled || !isEditorCommandTargetCurrent(current.target)))
            restoreViewport(current.cancelled ? viewportRef.current : current.start);
        }}
      >
        <Background />
      </ReactFlow>
    </div>
  );
}

export function MindFileEditor(scope: EditorPanelScope<"mind">) {
  return (
    <FileEditor
      {...scope}
      store={useMindProjectionStore}
      actions={mindActions}
      renderContent={(snapshot, reportError) => (
        <ReactFlowProvider key={snapshot.version.sessionId}>
          <MindEditor
            snapshot={snapshot}
            panelInstanceId={scope.panelInstanceId}
            isVisible={scope.isVisible}
            reportError={reportError}
          />
        </ReactFlowProvider>
      )}
    />
  );
}
