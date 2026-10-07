import { useResourceStore } from "@/features/core/resource";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import type { MindSnapshot } from "@/shared/types/domain/mind";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { deleteMindNodes, mindActions } from "./mindActions";
import { retainDocumentInput, resetDocumentInputs } from "./documentInputs";

vi.mock("@/services/mind/mindService", () => ({
  MindService: { read: vi.fn(), command: vi.fn() },
}));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { capturePublicationRevision: () => 0, submit: vi.fn() },
}));

const snapshot: MindSnapshot = {
  projectInstanceId: "project-a",
  path: "minds/Plan.yssbi-mind",
  kind: "mind",
  version: { sessionId: "mind-a", revision: 0 },
  dirty: false,
  content: {
    rootId: "root",
    nodes: [
      { id: "root", parentId: null, content: "Plan" },
      { id: "branch", parentId: "root", content: "Branch" },
      { id: "child", parentId: "branch", content: "Child" },
      { id: "sibling", parentId: "root", content: "Sibling" },
    ],
  },
};

beforeEach(() => {
  startProjectLifecycle(snapshot.projectInstanceId);
  useResourceStore.getState().clear();
  useResourceStore.getState().installFileSnapshot(snapshot);
  resetDocumentInputs();
});
afterEach(() => {
  vi.restoreAllMocks();
  resetDocumentInputs();
  useResourceStore.getState().clear();
});

it("deletes selected branches once while preserving the root", async () => {
  const edit = vi.spyOn(mindActions, "edit").mockResolvedValue(snapshot);
  await expect(deleteMindNodes(snapshot, ["root", "missing"])).resolves.toBeNull();
  expect(edit).not.toHaveBeenCalled();
  await deleteMindNodes(snapshot, ["root", "branch", "child", "sibling", "branch"]);
  expect(edit).toHaveBeenCalledExactlyOnceWith(snapshot.path, [
    { op: "remove_node", nodeId: "branch" },
    { op: "remove_node", nodeId: "sibling" },
  ]);
});

it("does not delete from a new project after waiting for unfinished topic text", async () => {
  const edit = vi.spyOn(mindActions, "edit").mockResolvedValue(snapshot);
  let complete!: () => void;
  retainDocumentInput(snapshot.projectInstanceId, snapshot.path, "topic", () => ({
    dirty: () => true,
    flush: () =>
      new Promise<void>((resolve) => {
        complete = resolve;
      }),
    prepareDiscard: () => () => {},
  }));
  const deletion = deleteMindNodes(snapshot, ["branch"]);
  startProjectLifecycle("project-b");
  complete();
  await expect(deletion).rejects.toMatchObject({ code: "stale_project_lifecycle" });
  expect(edit).not.toHaveBeenCalled();
});
