// @vitest-environment happy-dom
import {
  act,
  type ReactNode,
  type MouseEvent as ReactMouseEvent,
  type PointerEvent as ReactPointerEvent,
} from "react";
import { createRoot, type Root } from "react-dom/client";
import { beforeEach, afterEach, describe, expect, it, vi } from "vitest";
import type { ReactFlowProps, OnConnectEnd } from "@xyflow/react";
import type { EditorCanvasSession } from "@/features/application/editor";
import type { GraphProjectionSnapshot } from "@/features/core/graph/read";
import { FLOW_GRAPH_PATH, makeGraphFlowFixture } from "@/tests/helpers/graphFlowFixture";
import { GraphFlowCanvas } from "./GraphFlowCanvas";
import type { GraphFlowNode, GraphFlowEdge } from "./graphFlowModel";

type FlowProps = ReactFlowProps<GraphFlowNode, GraphFlowEdge>;
const mocks = vi.hoisted(() => ({
  flows: new Map<string, FlowProps>(),
  snapshot: { graphEntities: {}, graphMeta: {} } as GraphProjectionSnapshot,
  cancelConnection: vi.fn(),
  syncViewport: vi.fn(),
  viewportChange: vi.fn(),
  viewportCommit: vi.fn(),
  flowState: {
    nodesSelectionActive: false,
    userSelectionActive: false,
    userSelectionRect: null as unknown,
  },
}));
vi.mock("@/features/core/graph/read", () => ({
  useGraphRead: (select: (value: GraphProjectionSnapshot) => unknown) => select(mocks.snapshot),
}));
vi.mock("@xyflow/react", async (original) => ({
  ...(await original<typeof import("@xyflow/react")>()),
  ReactFlowProvider: ({ children }: { children: ReactNode }) => children,
  ReactFlow: (props: FlowProps) => {
    mocks.flows.set(props.id!, props);
    return props.children;
  },
  useStoreApi: () => ({
    getState: () => ({
      ...mocks.flowState,
      cancelConnection: mocks.cancelConnection,
      domNode: document.body,
      panZoom: { syncViewport: mocks.syncViewport },
    }),
    setState: (patch: object) => {
      Object.assign(mocks.flowState, patch);
    },
  }),
}));
vi.mock("./GraphFlowNode", () => ({ GraphFlowNode: () => null }));
vi.mock("./GraphFlowEdge", () => ({ GraphFlowEdge: () => null }));
vi.mock("./GraphFlowConnection", () => ({
  GraphFlowConnection: () => null,
  PendingFlowConnection: () => null,
}));
vi.mock("../../ContextMenu", () => ({ ConnectionContextMenu: () => null }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let root: Root;
let host: HTMLDivElement;
let canvas: EditorCanvasSession;
let active: boolean;
let cancelGesture: () => void;
const mutationResult = { status: "applied" as const };
const emptyEnd = { toNode: null, toHandle: null, isValid: false } as Parameters<OnConnectEnd>[1];
const flow = (id = "first") => mocks.flows.get(id)!;

function session(): EditorCanvasSession {
  return {
    workspace: {
      groupId: "group-a",
      activeGraph: { graphPath: FLOW_GRAPH_PATH, kind: "event" },
      selectedNodeIds: [],
      selectedConnectionIds: [],
      compileStatus: "uncompiled",
    },
    commands: {
      setSelectedNodeIds: vi.fn(),
      setSelectedConnectionIds: vi.fn(),
      breakConnectionsById: vi.fn(),
    } as unknown as EditorCanvasSession["commands"],
    interaction: {
      contextMenu: null,
      pendingConnection: null,
      setContextMenu: vi.fn(),
      setPendingConnection: vi.fn(),
      isInteractive: () => active,
      beginGesture: vi.fn((_type, cancel) => {
        if (!active) return null;
        let current = true;
        cancelGesture = () => {
          current = false;
          cancel();
        };
        return {
          isCurrent: () => current && active,
          finish: () => {
            current = false;
          },
        };
      }),
      insertRerouteAtConnection: vi.fn(async () => mutationResult),
      mutations: {
        submitNodePositions: vi.fn(async () => mutationResult),
        submitConnection: vi.fn(async () => mutationResult),
        disconnectPort: vi.fn(async () => mutationResult),
        insertRerouteAtConnection: vi.fn(async () => mutationResult),
        reportMutationFailure: vi.fn(),
      },
    },
  };
}
function render(twoPanels = false) {
  const props = {
    graphPath: FLOW_GRAPH_PATH,
    interactive: active,
    contextMenuActions: null,
    canvas,
    viewport: { x: 20, y: 40, scale: 2 },
    onViewportChange: mocks.viewportChange,
    onViewportCommit: mocks.viewportCommit,
    onContextMenu: vi.fn(),
  };
  act(() =>
    root.render(
      <>
        <GraphFlowCanvas {...props} panelInstanceId="first" groupId="group-a" />
        {twoPanels ? (
          <GraphFlowCanvas {...props} panelInstanceId="second" groupId="group-b" />
        ) : null}
      </>,
    ),
  );
}
function beginDrag(id = "first") {
  act(() =>
    flow(id).onNodeDragStart!(new MouseEvent("mousedown"), flow(id).nodes![0], [
      flow(id).nodes![0],
    ]),
  );
}
function move(x: number, id = "first") {
  act(() =>
    flow(id).onNodesChange!([
      { type: "position", id: "source", position: { x, y: 60 }, dragging: true },
    ]),
  );
}
async function endDrag(id = "first") {
  await act(async () =>
    flow(id).onNodeDragStop!(new MouseEvent("mouseup"), flow(id).nodes![0], [flow(id).nodes![0]]),
  );
}
beforeEach(() => {
  vi.clearAllMocks();
  Object.assign(mocks.flowState, {
    nodesSelectionActive: false,
    userSelectionActive: false,
    userSelectionRect: null,
  });
  mocks.flows.clear();
  mocks.snapshot = { graphEntities: { [FLOW_GRAPH_PATH]: makeGraphFlowFixture() }, graphMeta: {} };
  active = true;
  canvas = session();
  host = document.createElement("div");
  document.body.appendChild(host);
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  host.remove();
});

describe("React Flow canvas command bridge", () => {
  function panePointer(shiftKey = false) {
    const target = document.createElement("div");
    target.className = "react-flow__pane";
    return {
      target,
      button: 0,
      pointerId: 1,
      shiftKey,
      stopPropagation: vi.fn(),
      preventDefault: vi.fn(),
    } as unknown as ReactPointerEvent<HTMLDivElement>;
  }

  it("ignores viewport sync events and keeps a cancelled pan inert until the next gesture", () => {
    render();
    flow().onMoveStart!({ sync: true } as unknown as MouseEvent, { x: 20, y: 40, zoom: 2 });
    expect(canvas.interaction.beginGesture).not.toHaveBeenCalled();
    flow().onMoveStart!(new MouseEvent("mousedown", { button: 2 }), { x: 20, y: 40, zoom: 2 });
    flow().onViewportChange!({ x: 70, y: 90, zoom: 2 });
    expect(mocks.viewportChange).toHaveBeenLastCalledWith({ x: 70, y: 90, scale: 2 });
    act(() => cancelGesture());
    expect(mocks.viewportChange).toHaveBeenLastCalledWith({ x: 20, y: 40, scale: 2 });
    mocks.viewportChange.mockClear();
    flow().onMoveEnd!({ sync: true } as unknown as MouseEvent, { x: 20, y: 40, zoom: 2 });
    flow().onViewportChange!({ x: 150, y: 180, zoom: 2 });
    expect(mocks.viewportChange).not.toHaveBeenCalled();
    flow().onMoveEnd!(new MouseEvent("mouseup", { button: 2 }), { x: 150, y: 180, zoom: 2 });
    expect(mocks.viewportCommit).not.toHaveBeenCalled();
    expect(mocks.syncViewport).toHaveBeenLastCalledWith({ x: 20, y: 40, zoom: 2 });
    flow().onMoveStart!(new MouseEvent("mousedown", { button: 2 }), { x: 20, y: 40, zoom: 2 });
    flow().onViewportChange!({ x: 25, y: 45, zoom: 2 });
    flow().onMoveEnd!(new MouseEvent("mouseup", { button: 2 }), { x: 25, y: 45, zoom: 2 });
    expect(mocks.viewportChange).toHaveBeenLastCalledWith({ x: 25, y: 45, scale: 2 });
    expect(mocks.viewportCommit).toHaveBeenCalledOnce();
  });

  it("restores pre-pointer edge selection and prevents a cancelled box from completing or clicking", () => {
    canvas.workspace.selectedConnectionIds = ["original"];
    render();
    const event = panePointer();
    flow().onPointerDownCapture!(event);
    canvas = { ...canvas, workspace: { ...canvas.workspace, selectedConnectionIds: [] } };
    render();
    flow().onSelectionStart!(event);
    Object.assign(mocks.flowState, {
      userSelectionActive: true,
      userSelectionRect: { x: 1, y: 1, width: 40, height: 40 },
    });
    act(() => cancelGesture());
    expect(canvas.commands.setSelectedConnectionIds).toHaveBeenCalledWith(["original"], "group-a");
    expect(canvas.commands.setSelectedNodeIds).not.toHaveBeenCalled();
    expect(mocks.flowState).toMatchObject({ userSelectionActive: false, userSelectionRect: null });
    flow().onNodesChange!([{ type: "select", id: "source", selected: true }]);
    expect(canvas.commands.setSelectedNodeIds).not.toHaveBeenCalled();
    flow().onPointerUpCapture!(event);
    expect(event.stopPropagation).toHaveBeenCalledOnce();
    const click = { stopPropagation: vi.fn() } as unknown as ReactMouseEvent<HTMLDivElement>;
    flow().onClickCapture!(click);
    expect(click.stopPropagation).toHaveBeenCalledOnce();
  });

  it("keeps Shift pane clicks from reaching the native selection reset but still clears on plain clicks", () => {
    canvas.workspace.selectedNodeIds = ["source"];
    render();
    const event = panePointer(true);
    flow().onPointerDownCapture!(event);
    mocks.flowState.userSelectionRect = { width: 0, height: 0 };
    flow().onPointerUpCapture!(event);
    expect(mocks.flowState.userSelectionRect).toBeNull();
    expect(event.stopPropagation).toHaveBeenCalledOnce();
    flow().onPaneClick!(event);
    expect(canvas.commands.setSelectedNodeIds).not.toHaveBeenCalled();
    flow().onPointerDownCapture!(panePointer());
    flow().onPaneClick!({ shiftKey: false } as ReactMouseEvent);
    expect(canvas.commands.setSelectedNodeIds).toHaveBeenCalledWith([], "group-a");
  });

  it("keeps measured dimensions and unchanged node identities throughout a drag", () => {
    render();
    act(() =>
      flow().onNodesChange!([
        { type: "dimensions", id: "source", dimensions: { width: 160, height: 93 } },
        { type: "dimensions", id: "target", dimensions: { width: 180, height: 125 } },
      ]),
    );
    const stationary = flow().nodes![1];
    beginDrag();
    move(40);
    move(80);
    expect(flow().nodes![0]).toMatchObject({
      measured: { width: 160, height: 93 },
      dragging: true,
    });
    expect(flow().nodes![1]).toBe(stationary);
    act(() => cancelGesture());
    expect(flow().nodes![0].measured).toEqual({ width: 160, height: 93 });
    expect(flow().nodes![1]).toBe(stationary);
    active = false;
    render();
    act(() =>
      flow().onNodesChange!([
        { type: "dimensions", id: "target", dimensions: { width: 200, height: 140 } },
      ]),
    );
    expect(flow().nodes![1].measured).toEqual({ width: 200, height: 140 });
    act(() =>
      flow().onNodesChange!([
        { type: "dimensions", id: "target", dimensions: { width: 0, height: 0 } },
      ]),
    );
    expect(flow().nodes![1].measured).toEqual({ width: 200, height: 140 });
  });

  it("keeps pointer frames local and submits one batch at drag end", async () => {
    render(true);
    beginDrag();
    move(40);
    move(80);
    expect(flow().nodes![0].position).toEqual({ x: 80, y: 60 });
    expect(flow("second").nodes![0].position).toEqual({ x: 0, y: 0 });
    expect(mocks.snapshot.graphEntities[FLOW_GRAPH_PATH].nodes.source.position).toEqual({
      x: 0,
      y: 0,
    });
    expect(canvas.interaction.mutations.submitNodePositions).not.toHaveBeenCalled();
    await endDrag();
    expect(canvas.interaction.mutations.submitNodePositions).toHaveBeenCalledExactlyOnceWith(
      FLOW_GRAPH_PATH,
      [{ nodeId: "source", position: { x: 80, y: 60 } }],
    );
  });

  it("does not let an older mutation completion erase a newer position preview", async () => {
    let finishFirst!: (value: typeof mutationResult) => void;
    vi.mocked(canvas.interaction.mutations.submitNodePositions).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishFirst = resolve;
        }),
    );
    render();
    beginDrag();
    move(40);
    await endDrag();
    beginDrag();
    move(120);
    await act(async () => finishFirst(mutationResult));
    expect(flow().nodes![0].position.x).toBe(120);
    act(() => cancelGesture());
    await endDrag();
    expect(flow().nodes![0].position.x).toBe(0);
    expect(canvas.interaction.mutations.submitNodePositions).toHaveBeenCalledTimes(1);
  });

  it("routes connections through the draft and opens the palette only for a free-space drop", async () => {
    render();
    act(() =>
      flow().onConnectStart!(new MouseEvent("mousedown", { clientX: 10, clientY: 10 }), {
        nodeId: "source",
        handleId: "out",
        handleType: "source",
      }),
    );
    const candidate = {
      source: "source",
      target: "target",
      sourceHandle: "out",
      targetHandle: "right",
    };
    expect(flow().isValidConnection!(candidate)).toBe(true);
    await act(async () => {
      flow().onConnect!(candidate);
      flow().onConnectEnd!(new MouseEvent("mouseup", { clientX: 120, clientY: 40 }), emptyEnd);
    });
    expect(canvas.interaction.mutations.submitConnection).toHaveBeenCalledExactlyOnceWith({
      graphPath: FLOW_GRAPH_PATH,
      intent: "connect",
      sourcePinId: "out",
      targetPinId: "right",
    });
    expect(flow().edges).toHaveLength(1);
    expect(canvas.interaction.setContextMenu).not.toHaveBeenCalled();

    act(() => {
      flow().onConnectStart!(new MouseEvent("mousedown", { clientX: 10, clientY: 10 }), {
        nodeId: "source",
        handleId: "out",
        handleType: "source",
      });
      flow().onConnectEnd!(new MouseEvent("mouseup", { clientX: 200, clientY: 80 }), emptyEnd);
    });
    expect(canvas.interaction.setContextMenu).toHaveBeenCalledWith({
      x: 200,
      y: 80,
      visible: true,
    });
    expect(canvas.interaction.setPendingConnection).toHaveBeenCalledWith(
      expect.objectContaining({ id: "out" }),
    );
    vi.mocked(canvas.interaction.setContextMenu).mockClear();
    act(() => {
      flow().onConnectStart!(new MouseEvent("mousedown"), {
        nodeId: "source",
        handleId: "out",
        handleType: "source",
      });
      cancelGesture();
      flow().onConnectEnd!(new MouseEvent("mouseup", { clientX: 200 }), emptyEnd);
    });
    expect(canvas.interaction.setContextMenu).not.toHaveBeenCalled();
  });

  it("preserves the pre-click selection when requesting a reroute at the world coordinate", async () => {
    canvas.workspace.selectedNodeIds = ["source"];
    render();
    const event = {
      button: 0,
      detail: 1,
      clientX: 120,
      clientY: 100,
      preventDefault: vi.fn(),
    } as unknown as ReactMouseEvent;
    const edge = flow().edges![0];
    act(() => flow().onEdgeClick!(event, edge));
    canvas = {
      ...canvas,
      workspace: { ...canvas.workspace, selectedNodeIds: [], selectedConnectionIds: ["original"] },
    };
    render();
    await act(async () =>
      flow().onEdgeDoubleClick!({ ...event, detail: 2 } as ReactMouseEvent, edge),
    );
    expect(canvas.interaction.insertRerouteAtConnection).toHaveBeenCalledWith(
      "original",
      { x: 50, y: 30 },
      FLOW_GRAPH_PATH,
      "group-a",
      expect.objectContaining({
        before: { nodeIds: new Set(["source"]), connectionIds: new Set() },
        temporary: { nodeIds: new Set(), connectionIds: new Set(["original"]) },
      }),
    );
  });
});
