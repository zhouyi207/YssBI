// @vitest-environment happy-dom
import { beforeEach, expect, it, vi } from "vitest";
import { applyAssistantGraphTool } from "./assistantGraphTools";
import {
  HarnessGraphToolsService,
  type HarnessGraphToolRequest,
} from "@/services/assistant/harnessGraphToolsService";
import { useGraphDraftStore } from "@/features/core/graphDraft";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { resetGraphDraftCoordinator } from "@/features/application/graphDraft/graphDraftCoordinator";
import { undoEditorHistory } from "@/features/application/graphDraft/historyCoordinator";
import { GraphDraftService } from "@/services/nodeSystem/graphDraftService";
import {
  makeEditorProjectionFixture,
  makeGraphEditorSession,
} from "@/tests/helpers/editorProjectionFixtures";

vi.mock("@/features/application/graphProjection/graphProjectionLifecycle", () => ({
  loadGraphProjection: vi.fn(),
}));
vi.mock("@/features/application/execution/openInspectableResult", () => ({
  openInspectableResult: vi.fn(),
}));
const graphPath = "events/Main.yssbi-event";
const nodeId = "00000000-0000-0000-0000-000000000001";
const projection = () => makeEditorProjectionFixture({ graphPath, nodeId }).projection;
const current = () => useGraphDraftStore.getState().sessions[graphPath];
const request = (
  requestId: string,
  capabilityId = "apply_graph_edit",
): HarnessGraphToolRequest => ({
  requestId,
  capabilityId,
  graphPath,
  projectInstanceId: "project-1",
  sessionId: "session-1",
});

beforeEach(() => {
  vi.restoreAllMocks();
  clearProjectLifecycle();
  startProjectLifecycle("project-1");
  resetGraphDraftCoordinator();
  useGraphDraftStore.getState().clear();
  useGraphProjectionStore.setState({ graphEntities: {} });
  const initial = makeGraphEditorSession(projection());
  initial.document.nodes[nodeId] = {
    id: nodeId,
    node_type: "tests.projected-node",
    position: { x: 0, y: 0 },
    parameters: {},
    user_label: null,
  };
  useGraphDraftStore.getState().install(graphPath, initial);
  useGraphProjectionStore.getState().replaceProjection(graphPath, projection());
  vi.spyOn(HarnessGraphToolsService, "complete").mockResolvedValue();
  vi.spyOn(HarnessGraphToolsService, "claim").mockResolvedValue(true);
});

it("applies queued AI edits to the current draft and retains normal undo history", async () => {
  const manual = structuredClone(current().document);
  manual.nodes[nodeId].parameters.manual = "unsaved";
  useGraphDraftStore
    .getState()
    .applyTransform(graphPath, { changed: true, document: manual, projection: projection() });
  let release!: () => void;
  const gate = new Promise<void>((resolve) => {
    release = resolve;
  });
  const prepare = vi
    .spyOn(HarnessGraphToolsService, "prepare")
    .mockImplementation(async (r, document) => {
      if (r.requestId === "first") await gate;
      const next = structuredClone(document);
      if (r.requestId === "second") expect(next.nodes[nodeId].parameters.first).toBe(true);
      next.nodes[nodeId].parameters[r.requestId] = true;
      return { type: "draft", update: { changed: true, document: next, projection: projection() } };
    });
  const first = applyAssistantGraphTool(request("first"), () => true);
  const second = applyAssistantGraphTool(request("second"), () => true);
  release();
  await Promise.all([first, second]);
  expect(prepare).toHaveBeenCalledTimes(2);
  expect(current().document.nodes[nodeId].parameters).toEqual({
    manual: "unsaved",
    first: true,
    second: true,
  });
  expect(current().undoStack).toHaveLength(3);
  expect(current().saveDirty).toBe(true);
  expect(HarnessGraphToolsService.complete).toHaveBeenCalledWith("first", true);
  vi.spyOn(GraphDraftService, "resolve").mockResolvedValue(projection());
  await undoEditorHistory(graphPath);
  await undoEditorHistory(graphPath);
  expect(current().document.nodes[nodeId].parameters).toEqual({ manual: "unsaved" });
});

it("does not install a late graph result after project replacement or a rejected adoption", async () => {
  vi.spyOn(HarnessGraphToolsService, "prepare").mockImplementation(async (_r, document) => {
    startProjectLifecycle("project-2");
    const next = structuredClone(document);
    next.nodes[nodeId].parameters.stale = true;
    return { type: "draft", update: { changed: true, document: next, projection: projection() } };
  });
  await applyAssistantGraphTool(request("stale"), () => true);
  expect(current().document.nodes[nodeId].parameters).toEqual({});
  expect(HarnessGraphToolsService.complete).toHaveBeenCalledWith("stale", false);
  startProjectLifecycle("project-1");
  vi.mocked(HarnessGraphToolsService.prepare).mockImplementation(async (_r, document) => ({
    type: "draft",
    update: { changed: true, document, projection: projection() },
  }));
  vi.mocked(HarnessGraphToolsService.claim).mockResolvedValue(false);
  await applyAssistantGraphTool(request("cancelled"), () => true);
  expect(current().undoStack).toHaveLength(0);
  expect(HarnessGraphToolsService.complete).toHaveBeenCalledWith("cancelled", false);
});
