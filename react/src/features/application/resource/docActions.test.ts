import { beforeEach, expect, it, vi } from "vitest";
import { DocService } from "@/services/doc/docService";
import type { DocSnapshot } from "@/shared/types/domain/doc";
import { buildFileResourceMeta, useResourceStore } from "@/features/core/resource";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { docActions } from "./docActions";

const mocks = vi.hoisted(() => ({ publicationRevision: 0 }));
vi.mock("@/services/doc/docService", () => ({
  DocService: { read: vi.fn(), command: vi.fn() },
}));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: {
    capturePublicationRevision: () => mocks.publicationRevision,
    submit: vi.fn(),
  },
}));

beforeEach(() => {
  vi.clearAllMocks();
  mocks.publicationRevision = 0;
  startProjectLifecycle("project-a");
  useResourceStore.getState().clear();
});

it("does not restore a file removed from the index during its first read", async () => {
  const snapshot: DocSnapshot = {
    projectInstanceId: "project-a",
    path: "docs/Report.md",
    version: { sessionId: "document-a", revision: 0 },
    kind: "doc",
    content: "saved",
    dirty: false,
  };
  useResourceStore.getState().setSnapshot({
    resources: [buildFileResourceMeta("doc", snapshot.path, "Report", { revision: 0 })],
  });
  let complete!: (snapshot: DocSnapshot) => void;
  vi.mocked(DocService.read).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const reading = docActions.load(snapshot.path);
  useResourceStore
    .getState()
    .setSnapshot({ resources: [], documents: {}, fileSnapshots: { mind: {}, doc: {} } });
  complete(snapshot);
  await expect(reading).rejects.toThrow("Document read lifecycle ended");
  expect(useResourceStore.getState().fileSnapshots.doc).toEqual({});
  expect(useResourceStore.getState().documents).toEqual({});
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
  expect(useResourceStore.getState().fileSnapshots.doc).toEqual({});

  const replacing = docActions.load(snapshot.path);
  startProjectLifecycle("project-b");
  complete(snapshot);
  await expect(replacing).rejects.toMatchObject({ code: "stale_project_lifecycle" });
  expect(useResourceStore.getState().fileSnapshots.doc).toEqual({});
});

it("shares concurrent reads instead of invalidating an awaiting editor's first read", async () => {
  const snapshot: DocSnapshot = {
    projectInstanceId: "project-a",
    path: "docs/Report.md",
    version: { sessionId: "document-a", revision: 0 },
    kind: "doc",
    content: "saved",
    dirty: false,
  };
  let complete!: (snapshot: DocSnapshot) => void;
  vi.mocked(DocService.read).mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        complete = resolve;
      }),
  );
  const firstPane = docActions.load(snapshot.path);
  const secondPane = docActions.load(snapshot.path);
  expect(DocService.read).toHaveBeenCalledOnce();
  complete(snapshot);

  await expect(firstPane).resolves.toBe(snapshot);
  await expect(secondPane).resolves.toBe(snapshot);
  expect(useResourceStore.getState().fileSnapshots.doc[snapshot.path]).toBe(snapshot);
});

it("starts a fresh read when publication advances during an older read", async () => {
  const snapshot: DocSnapshot = {
    projectInstanceId: "project-a",
    path: "docs/Report.md",
    version: { sessionId: "document-a", revision: 0 },
    kind: "doc",
    content: "saved",
    dirty: false,
  };
  const responses: ((snapshot: DocSnapshot) => void)[] = [];
  vi.mocked(DocService.read).mockImplementation(
    () =>
      new Promise((resolve) => {
        responses.push(resolve);
      }),
  );
  const oldRead = docActions.load(snapshot.path);
  mocks.publicationRevision = 1;
  const refresh = docActions.load(snapshot.path);
  expect(DocService.read).toHaveBeenCalledTimes(2);
  const latest = { ...snapshot, content: "updated", version: { ...snapshot.version, revision: 1 } };
  responses[1](latest);
  await expect(refresh).resolves.toBe(latest);
  responses[0](snapshot);
  await expect(oldRead).resolves.toBe(latest);
  expect(useResourceStore.getState().fileSnapshots.doc[snapshot.path]).toBe(latest);
});
