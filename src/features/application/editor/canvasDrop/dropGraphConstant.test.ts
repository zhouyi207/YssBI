import { beforeEach, expect, it, vi } from "vitest";
import type { GraphConstantDragState } from "@/features/core/dnd";
import { DRAG_TYPES } from "@/features/core/dnd";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { editorViewportScope, setViewportLive, useViewportStore } from "@/features/core/viewport";
import { resetGraphEditCoordinator } from "@/features/application/graphEditing/graphEditCoordinator";
import { GraphEditingService } from "@/services/nodeSystem/graphEditingService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";
import { dropGraphConstantIntoCanvas } from "./dropGraphConstant";

const mocks = vi.hoisted(() => ({
  captureTarget: vi.fn(),
  isTargetCurrent: vi.fn(),
}));
vi.mock("../editorCommandFocus", () => ({
  captureEditorCommandTarget: mocks.captureTarget,
  isEditorCommandTargetCurrent: mocks.isTargetCurrent,
}));

const graphPath = "events/constants.yssbi-event";
const constantId = "00000000-0000-0000-0000-000000000004";
const canvas = {
  getBoundingClientRect: () => ({ left: 100, top: 50, right: 900, bottom: 650 }),
} as HTMLElement;
const dragState: GraphConstantDragState = {
  type: DRAG_TYPES.GRAPH_CONSTANT,
  graphPath,
  constantId,
  name: "Threshold",
  x: 400,
  y: 250,
};

beforeEach(() => {
  vi.restoreAllMocks();
  clearProjectLifecycle();
  startProjectLifecycle("constant-project");
  resetGraphEditCoordinator();
  useGraphProjectionStore.getState().clear();
  useViewportStore.getState().clear();
  const session = makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection);
  session.document.constants = {
    [constantId]: {
      id: constantId,
      name: "Threshold",
      dataType: { kind: "Scalar", inner: "Numeric" },
      dataValue: { Integer: "7" },
    },
  };
  useGraphProjectionStore.getState().install(graphPath, session);
  mocks.captureTarget.mockReturnValue({
    panelInstanceId: "editor-1",
    groupId: "group-1",
    resourceRef: graphPath,
    resourceKind: "event_graph",
  });
  mocks.isTargetCurrent.mockReturnValue(true);
});

it("adds the existing constant at the target pane's zoomed and panned drop position", async () => {
  setViewportLive(editorViewportScope("group-1", graphPath), { x: 40, y: -20, scale: 2 });
  const transform = vi
    .spyOn(GraphEditingService, "transform")
    .mockImplementation(async (_project, _path, _locale, version) => ({
      ...makeGraphEditorSession(makeEditorProjectionFixture({ graphPath }).projection),
      document: structuredClone(useGraphProjectionStore.getState().sessions[graphPath].document),
      changed: true,
      editing: {
        version: { ...version, revision: String(BigInt(version.revision) + 1n) },
        dirty: true,
        canUndo: true,
        canRedo: false,
      },
    }));

  await expect(
    dropGraphConstantIntoCanvas(canvas, "editor-1", "group-1", graphPath, dragState),
  ).resolves.toBe(true);
  expect(mocks.captureTarget).toHaveBeenCalledWith("editor-1");
  expect(transform).toHaveBeenCalledOnce();
  expect(transform.mock.calls[0][4]).toEqual({
    type: "insertConstantReference",
    payload: { id: constantId, position: { x: 130, y: 110 } },
  });
});

it("does not edit a different graph or an unavailable drop target", async () => {
  const transform = vi.spyOn(GraphEditingService, "transform");
  for (const invalidDrag of [
    { ...dragState, graphPath: "events/other.yssbi-event" },
    { ...dragState, constantId: "deleted-constant" },
    { ...dragState, x: 950 },
  ]) {
    await expect(
      dropGraphConstantIntoCanvas(canvas, "editor-1", "group-1", graphPath, invalidDrag),
    ).resolves.toBe(false);
  }
  mocks.isTargetCurrent.mockReturnValue(false);
  await expect(
    dropGraphConstantIntoCanvas(canvas, "editor-1", "group-1", graphPath, dragState),
  ).resolves.toBe(false);
  mocks.isTargetCurrent.mockReturnValue(true);
  useGraphProjectionStore.getState().beginSave(graphPath);
  await expect(
    dropGraphConstantIntoCanvas(canvas, "editor-1", "group-1", graphPath, dragState),
  ).resolves.toBe(false);
  expect(transform).not.toHaveBeenCalled();
});
