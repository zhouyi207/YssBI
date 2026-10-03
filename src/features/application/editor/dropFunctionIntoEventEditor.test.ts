import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { DragEndEvent } from "@dnd-kit/core";
import { logger } from "@/utils/frontendLogger";
import { canvasDropHandlerStore } from "@/features/core/sidebarDrag";
import {
  captureProjectIdentity,
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildFunctionGraphResourceDragState,
  tryDropFunctionIntoCanvas,
  type CanvasDropTarget,
} from "./dropFunctionIntoEventEditor";
import {
  beginActivityEditorDrag,
  executeEditorDragEnd,
  finishActivityEditorDrag,
} from "./editorDragDropActions";

const { activate, openFallback } = vi.hoisted(() => ({
  activate: vi.fn(),
  openFallback: vi.fn(),
}));
vi.mock("@/modules/workbench/public", () => ({ workbenchLayoutControl: { activate } }));
vi.mock("./handleGraphResourceDrop", () => ({ handleGraphResourceDrop: openFallback }));
vi.mock("./sidebarSpawnDropPolicy", () => ({ isSidebarSpawnDropAllowed: () => true }));

const target: CanvasDropTarget = {
  panelInstanceId: "function-drop-panel",
  groupId: "function-drop-group",
  graphPath: "events/Main.yssbi-event",
  graphKind: "event_graph",
};
const drag = buildFunctionGraphResourceDragState("functions/Read.yssbi-function", "Read", 20, 30);
const unregisterHandlers: Array<() => void> = [];

beforeEach(() => {
  vi.clearAllMocks();
  startProjectLifecycle("project-a");
});
afterEach(() => {
  vi.restoreAllMocks();
  finishActivityEditorDrag();
  for (const unregister of unregisterHandlers.splice(0)) unregister();
  clearProjectLifecycle();
});

it("uses the current canvas handler after activating its panel", async () => {
  let finishActivation!: (activated: boolean) => void;
  activate.mockReturnValueOnce(
    new Promise<boolean>((resolve) => {
      finishActivation = resolve;
    }),
  );
  const previous = vi.fn(async () => false);
  const current = vi.fn(async () => true);
  unregisterHandlers.push(canvasDropHandlerStore.registerHandler(target.panelInstanceId, previous));
  const dropping = tryDropFunctionIntoCanvas(target, drag, captureProjectIdentity());
  unregisterHandlers.push(canvasDropHandlerStore.registerHandler(target.panelInstanceId, current));
  finishActivation(true);

  await expect(dropping).resolves.toBe(true);
  expect(previous).not.toHaveBeenCalled();
  expect(current).toHaveBeenCalledExactlyOnceWith(drag);
});

it("stops handler dispatch and resource fallback after activation crosses a project lifecycle", async () => {
  const reportFailure = vi.spyOn(logger.graph, "error").mockImplementation(() => {});
  let finishActivation!: (activated: boolean) => void;
  activate.mockReturnValueOnce(
    new Promise<boolean>((resolve) => {
      finishActivation = resolve;
    }),
  );
  const handler = vi.fn(async () => true);
  unregisterHandlers.push(canvasDropHandlerStore.registerHandler(target.panelInstanceId, handler));
  const event = {
    active: { data: { current: drag } },
    over: { data: { current: target } },
    activatorEvent: { clientX: drag.x, clientY: drag.y },
    delta: { x: 0, y: 0 },
  } as unknown as DragEndEvent;
  expect(beginActivityEditorDrag(event)).toBe(true);
  const dropping = executeEditorDragEnd(event, { finishSidebarDrag: finishActivityEditorDrag });
  expect(activate).toHaveBeenCalledExactlyOnceWith(target.panelInstanceId);
  startProjectLifecycle("project-b");
  finishActivation(true);

  await dropping;
  expect(handler).not.toHaveBeenCalled();
  expect(openFallback).not.toHaveBeenCalled();
  expect(reportFailure).not.toHaveBeenCalled();
});
