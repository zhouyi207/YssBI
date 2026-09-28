import { beforeEach, expect, it, vi } from "vitest";
import { DocService } from "@/services/doc/docService";
import type { DocSnapshot } from "@/shared/types/domain/doc";
import { useDocProjectionStore } from "@/features/core/resource/docProjectionStore";
import { useDocumentStateStore, useResourceStore } from "@/features/core/resource";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { docActions } from "./docActions";

vi.mock("@/services/doc/docService", () => ({
  DocService: { read: vi.fn(), command: vi.fn() },
}));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: { capturePublicationRevision: () => 0, submit: vi.fn() },
}));

beforeEach(() => {
  vi.clearAllMocks();
  startProjectLifecycle("project-a");
  useDocProjectionStore.getState().clear();
  useDocumentStateStore.getState().clear();
  useResourceStore.getState().clear();
});

it("does not restore released or previous-project documents from late reads", async () => {
  const snapshot: DocSnapshot = {
    projectInstanceId: "project-a",
    path: "docs/Report.md",
    version: { sessionId: "document-a", revision: 0 },
    kind: "doc",
    content: "saved",
    dirty: false,
  };
  let complete!: (snapshot: DocSnapshot) => void;
  vi.mocked(DocService.read).mockImplementation(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const closing = docActions.load(snapshot.path);
  docActions.release(snapshot.path);
  complete(snapshot);
  await expect(closing).rejects.toThrow("Document read lifecycle ended");
  expect(useDocProjectionStore.getState().documents).toEqual({});

  const replacing = docActions.load(snapshot.path);
  startProjectLifecycle("project-b");
  complete(snapshot);
  await expect(replacing).rejects.toMatchObject({ code: "stale_project_lifecycle" });
  expect(useDocProjectionStore.getState().documents).toEqual({});
});
