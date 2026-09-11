// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { EditorCanvasScope } from "./editorCanvasTypes";
import { useCanvasInteraction } from "./useCanvasInteraction";
import {
  cancelCanvasInteraction,
  clearCanvasInteractionProject,
} from "@/features/core/canvas/canvasInteractionCleanup";
import {
  getCanvasInteraction,
  useGraphInteractionStore,
} from "@/features/core/graphInteraction/graphInteractionStore";
import type { CanvasInteractionHandlers } from "@/features/core/canvas/canvasMutationContracts";

const mocks = vi.hoisted(() => ({
  target: null as null | {
    panelInstanceId: string;
    groupId: string;
    resourceRef: string;
    resourceKind: "event";
  },
  prepare: vi.fn(),
  selection: { nodeIds: new Set<string>(), connectionIds: new Set<string>() },
  setConnections: vi.fn(),
}));
vi.mock("./editorCommandFocus", () => ({
  captureActiveEditorCommandTarget: () => mocks.target,
  isEditorCommandTargetCurrent: (target: unknown) => target === mocks.target,
}));
vi.mock("./editorGroupInteraction", () => ({ prepareEditorGroupForInteraction: mocks.prepare }));
vi.mock("@/modules/workbench/public", () => ({
  getEditorGroupGraphSelection: () => mocks.selection,
  updateEditorGroupSelectedConnectionIds: mocks.setConnections,
}));
const scope: EditorCanvasScope = {
  panelInstanceId: "first",
  groupId: "a",
  graphPath: "graph",
  graphKind: "event",
};
let root: Root;
let host: HTMLDivElement;
let interaction: ReturnType<typeof useCanvasInteraction>;
let setNodes: ReturnType<typeof vi.fn<(ids: string[], groupId?: string) => void>>;
let handlers: CanvasInteractionHandlers;
function Harness({ enabled }: { enabled: boolean }) {
  interaction = useCanvasInteraction({ scope, enabled, handlers, setSelectedNodeIds: setNodes });
  return null;
}
function render(enabled = true) {
  act(() => root.render(<Harness enabled={enabled} />));
}
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
beforeEach(() => {
  vi.clearAllMocks();
  useGraphInteractionStore.setState({ interactions: {} });
  mocks.target = {
    panelInstanceId: "first",
    groupId: "a",
    resourceRef: "graph",
    resourceKind: "event",
  };
  setNodes = vi.fn<(ids: string[], groupId?: string) => void>();
  handlers = {
    submitNodePositions: vi.fn(),
    submitConnection: vi.fn(),
    disconnectPort: vi.fn(),
    insertRerouteAtConnection: vi.fn(),
    reportMutationFailure: vi.fn(),
  };
  host = document.createElement("div");
  root = createRoot(host);
});
afterEach(() => {
  act(() => root.unmount());
  clearCanvasInteractionProject();
});

it("invalidates a cancelled renderer gesture and refuses stale callbacks without stealing focus", () => {
  render();
  const cancel = vi.fn();
  let lease: ReturnType<typeof interaction.beginGesture>;
  act(() => {
    lease = interaction.beginGesture("draggingNodes", cancel);
  });
  expect(lease!.isCurrent()).toBe(true);
  act(() => {
    cancelCanvasInteraction("graph", "a");
  });
  expect(cancel).toHaveBeenCalledOnce();
  expect(lease!.isCurrent()).toBe(false);
  mocks.target = {
    panelInstanceId: "second",
    groupId: "b",
    resourceRef: "graph",
    resourceKind: "event",
  };
  mocks.prepare.mockClear();
  expect(interaction.beginGesture("draggingNodes", vi.fn())).toBeNull();
  expect(mocks.prepare).not.toHaveBeenCalled();
});

it("releases gesture ownership when the canvas becomes a saving or inactive preview", () => {
  render();
  const cancel = vi.fn();
  act(() => {
    interaction.beginGesture("drawingConnection", cancel);
  });
  render(false);
  expect(cancel).toHaveBeenCalledOnce();
  expect(getCanvasInteraction(useGraphInteractionStore.getState(), "graph", "a").type).toBe("idle");
  expect(interaction.beginGesture("selecting", vi.fn())).toBeNull();
});

it("restores a failed reroute selection only while that exact selection and target still apply", async () => {
  render();
  const before = { nodeIds: new Set(["node"]), connectionIds: new Set<string>() };
  const temporary = { nodeIds: new Set<string>(), connectionIds: new Set(["edge"]) };
  mocks.selection = temporary;
  vi.mocked(handlers.insertRerouteAtConnection).mockResolvedValueOnce({ status: "failed" });
  await act(async () => {
    await interaction.insertRerouteAtConnection("edge", { x: 10, y: 20 }, "graph", "a", {
      before,
      temporary,
    });
  });
  expect(setNodes).toHaveBeenCalledExactlyOnceWith(["node"], "a");
  let finish!: (value: { status: "failed" }) => void;
  vi.mocked(handlers.insertRerouteAtConnection).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const pending = interaction.insertRerouteAtConnection("edge", { x: 10, y: 20 }, "graph", "a", {
    before,
    temporary,
  });
  mocks.selection = { nodeIds: new Set(["new-selection"]), connectionIds: new Set() };
  await act(async () => {
    finish({ status: "failed" });
    await pending;
  });
  expect(setNodes).toHaveBeenCalledTimes(1);
});
