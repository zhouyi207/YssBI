import { beforeEach, expect, it, vi } from "vitest";
import { openFileInEditor } from "./openFileInEditor";
const mocks = vi.hoisted(() => ({
  open: vi.fn(),
  reveal: vi.fn(),
  handled: new Error("layout feedback already presented"),
}));
vi.mock("./openEditorPanel", () => ({
  openEditorPanel: mocks.open,
  isEditorOpenRejectionHandled: (error: unknown) => error === mocks.handled,
}));
vi.mock("./activateEditorPanelAndSyncSession", () => ({
  activateEditorPanelAndSyncSession: mocks.reveal,
}));
vi.mock("./openGraphInEditor", () => ({ openGraphInEditor: vi.fn() }));
beforeEach(() => {
  vi.clearAllMocks();
});
it("contains an editor-open rejection whose feedback was already presented", async () => {
  mocks.open.mockRejectedValueOnce(mocks.handled);
  await expect(openFileInEditor("charts/Summary.yssbi-chart", "chart")).resolves.toBeUndefined();
  expect(mocks.reveal).not.toHaveBeenCalled();
});
