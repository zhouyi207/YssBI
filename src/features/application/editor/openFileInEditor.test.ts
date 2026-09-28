import { beforeEach, expect, it, vi } from "vitest";
import { openFileInEditor } from "./openFileInEditor";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
const mocks = vi.hoisted(() => ({
  open: vi.fn(),
  activate: vi.fn(),
  handled: new Error("layout feedback already presented"),
}));
vi.mock("./openEditorPanel", () => ({
  openEditorPanel: mocks.open,
  isEditorOpenRejectionHandled: (error: unknown) => error === mocks.handled,
}));
vi.mock("./activateEditorPanelAndSyncSession", () => ({
  activateEditorPanelAndSyncSession: mocks.activate,
}));
vi.mock("@/features/core/chart/chartDocumentStore", () => ({
  useChartDocumentStore: { getState: () => ({ documents: { "charts/Summary.yssbi-chart": {} } }) },
}));
vi.mock("@/features/application/projectCommandContext", () => ({
  captureProjectCommandContext: () => ({ assertCurrent() {}, projectInstanceId: "project-a" }),
}));
vi.mock("./openGraphInEditor", () => ({ openGraphInEditor: vi.fn() }));
vi.mock("@/features/application/resource/mindActions", () => ({ mindActions: { load: vi.fn() } }));
vi.mock("@/features/application/resource/docActions", () => ({ docActions: { load: vi.fn() } }));
beforeEach(() => {
  vi.clearAllMocks();
  startProjectLifecycle("project-a");
});
it("contains an editor-open rejection whose feedback was already presented", async () => {
  mocks.open.mockRejectedValueOnce(mocks.handled);
  await expect(openFileInEditor("charts/Summary.yssbi-chart", "chart")).resolves.toBeUndefined();
  expect(mocks.activate).not.toHaveBeenCalled();
});
