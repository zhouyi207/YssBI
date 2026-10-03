import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { useEditorStore } from "@/features/core/editor";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import type { WorkbenchEditorPanelInfo } from "@/modules/workbench/internal/layout/workbenchRead";
import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { revealGraphProblem } from "./revealGraphProblem";

const mocks = vi.hoisted(() => ({
  reveal: vi.fn(),
  activate: vi.fn(),
  revealDetails: vi.fn(),
  getPanel: vi.fn(),
  findPanels: vi.fn(),
}));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutRead: {
    isReady: true,
    getPanel: mocks.getPanel,
    findEditorPanelsByResource: mocks.findPanels,
    getMutationRevision: () => 1,
  },
  workbenchLayoutControl: { reveal: mocks.reveal, activate: mocks.activate },
  revealWorkbenchView: mocks.revealDetails,
  updateEditorGroupSelectedNodeIds: vi.fn(),
  updateEditorGroupSelectedConnectionIds: vi.fn(),
}));
vi.mock("./openGraphResource", () => ({ openGraphResource: vi.fn() }));

const graphPath = "events/Problems.yssbi-event";
const nodeId = "problem-node";
const panel: WorkbenchEditorPanelInfo = {
  panelInstanceId: "problems-panel",
  groupId: "problems-group",
  component: "EditorResource",
  metadata: { role: "editor", resourceKind: "event_graph", resourceRef: graphPath },
  active: true,
  visible: true,
  location: { type: "grid" },
};

beforeEach(() => {
  vi.resetAllMocks();
  startProjectLifecycle("problems-project-a");
  useResourceStore.getState().clear();
  useEditorStore.getState().clearDetailFocus();
  installGraphProjectionFixture(
    graphPath,
    makeEditorProjectionFixture({ graphPath, nodeId }).projection,
  );
  mocks.reveal.mockResolvedValue(true);
  mocks.activate.mockResolvedValue(true);
  mocks.getPanel.mockReturnValue(panel);
  mocks.findPanels.mockReturnValue([panel]);
  // These orchestration cases end before any canvas or field is available.
  vi.stubGlobal("document", { querySelector: () => null });
  vi.stubGlobal("CSS", { escape: (value: string) => value });
  vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
    callback(0);
    return 1;
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  useResourceStore.getState().clear();
  useEditorStore.getState().clearDetailFocus();
  clearProjectLifecycle();
});

async function pauseAtDetails() {
  let finishDetails!: () => void;
  let detailsStarted!: () => void;
  const started = new Promise<void>((resolve) => {
    detailsStarted = resolve;
  });
  const pending = new Promise<void>((resolve) => {
    finishDetails = resolve;
  });
  mocks.revealDetails.mockImplementationOnce(() => {
    detailsStarted();
    return pending;
  });
  const revealing = revealGraphProblem(graphPath, { kind: "node", nodeId }, panel.groupId);
  await started;
  expect(mocks.revealDetails).toHaveBeenCalledExactlyOnceWith("details");
  expect(mocks.activate).not.toHaveBeenCalled();
  return { finishDetails, revealing };
}

it("stops graph activation when Details reveal crosses a project lifecycle", async () => {
  const { finishDetails, revealing } = await pauseAtDetails();
  startProjectLifecycle("problems-project-b");
  finishDetails();

  await expect(revealing).resolves.toBe(false);
  expect(mocks.activate).not.toHaveBeenCalled();
});

it("stops graph activation when the target panel changes resource during Details reveal", async () => {
  const { finishDetails, revealing } = await pauseAtDetails();
  mocks.getPanel.mockReturnValue({
    ...panel,
    metadata: { ...panel.metadata, resourceRef: "events/Other.yssbi-event" },
  });
  finishDetails();

  const revealed = await revealing;
  expect(mocks.activate).not.toHaveBeenCalled();
  expect(revealed).toBe(false);
});
