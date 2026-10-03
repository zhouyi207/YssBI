import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  prepareProjectSnapshotCommit,
  commitPreparedProjectSnapshot,
  collectDeletedResourceKeys,
} from "./projectPublicationSnapshot";
import type { ProjectSnapshotPreparation } from "./projectPublicationCoordinator";

import {
  buildFileResourceMeta,
  markResourceLoaded,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import {
  captureProjectIdentity,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { createFileTextInput } from "@/features/application/resource/fileTextInput";
import { createFileActions } from "@/features/application/resource/createFileActions";
import {
  hasPendingDocumentInput,
  retainDocumentInput,
  resetDocumentInputs,
} from "@/features/application/resource/documentInputs";
import type { DocEdit, DocSnapshot } from "@/shared/types/domain/doc";

vi.mock("./editorLayoutPublicationCommit", () => ({
  commitEditorLayoutPublication: (
    _moves: unknown,
    _resources: unknown,
    _deleted: unknown,
    commit: () => void,
  ) => commit(),
}));
const saved: DocSnapshot = {
  projectInstanceId: "project-a",
  path: "docs/Report.md",
  kind: "doc",
  content: "saved",
  dirty: false,
  version: { sessionId: "doc-a", revision: 0 },
};
beforeEach(() => {
  resetDocumentInputs();
  startProjectLifecycle("project-a");
  useResourceStore.getState().clear();
  useResourceStore
    .getState()
    .setSnapshot({ resources: [buildFileResourceMeta("doc", saved.path, "Report")] });
  markResourceLoaded({ id: saved.path, kind: "doc" });
  useResourceStore.getState().installFileSnapshot(saved);
});
afterEach(resetDocumentInputs);
function setup() {
  const edit = vi.fn(async () => saved);
  const input = retainDocumentInput(saved.projectInstanceId, saved.path, "doc-a:markdown", () =>
    createFileTextInput(
      saved,
      saved.content,
      (markdown): DocEdit => ({ op: "set_markdown", markdown }),
      { edit, getSnapshot: (path) => useResourceStore.getState().fileSnapshots.doc[path] },
    ),
  );
  input.change("unsaved text");
  const plan: ProjectSnapshotPreparation = {
    ...captureProjectIdentity(),
    publicationRevision: 1,
    activityPanels: [],
    index: {
      projectInstanceId: "project-a",
      publicationRevision: 1,
      projectName: "Project",
      exportTime: "",
      eventGraphs: [],
      functionGraphs: [],
      charts: [],
      minds: [],
      docs: [],
      databases: [],
    },
    graphSessions: new Map(),
    chartDocuments: new Map(),
    pathRemaps: new Map(),
    filePathRemaps: new Map(),
    deletedResources: new Set(),
  };
  return { input, edit, plan };
}

it("preserves externally missing text until explicit discard, without restoring index membership", async () => {
  const { input, plan } = setup();
  await commitPreparedProjectSnapshot(prepareProjectSnapshotCommit(plan));
  expect(useResourceStore.getState().fileSnapshots.doc[saved.path]).toEqual(saved);
  expect(hasPendingDocumentInput(saved.path)).toBe(true);
  input.change("still recoverable");
  expect(
    useResourceStore.getState().resources[resourceKey({ id: saved.path, kind: "doc" })],
  ).toMatchObject({ exists: false, hasDirtyDocument: true });
  const command = vi.fn();
  const actions = createFileActions<"doc", DocEdit>("doc", {
    read: vi.fn(),
    command,
  });
  await actions.discard(saved.path, saved.version);
  expect(command).not.toHaveBeenCalled();
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  expect(useResourceStore.getState().fileSnapshots.doc[saved.path]).toBeUndefined();
});

it("releases dirty buffers only when a deletion receipt authorizes removal", async () => {
  const { plan } = setup();
  const operationId = "00000000-0000-0000-0000-000000000123";
  const deletedResources = collectDeletedResourceKeys(plan.index, [
    {
      projectInstanceId: saved.projectInstanceId,
      operationId,
      publicationRevision: 1,
      moves: [],
      projectionReplacements: [],
      projectionStatus: { status: "complete", expectedGraphPaths: [] },
      deltas: [
        {
          resource: { kind: "doc", key: saved.path },
          fromRevision: 0,
          toRevision: 1,
          causedBy: operationId,
          payload: {
            kind: "resource_lifecycle",
            patch: {
              before: { kind: "doc", path: saved.path, name: "Report", revision: 0 },
              after: null,
            },
          },
        },
      ],
    },
  ]);
  const observed: unknown[] = [];
  const key = resourceKey({ id: saved.path, kind: "doc" });
  const stop = useResourceStore.subscribe((state) =>
    observed.push({
      content: state.fileSnapshots.doc[saved.path],
      document: state.documents[key],
      resource: state.resources[key],
    }),
  );
  try {
    await commitPreparedProjectSnapshot(
      prepareProjectSnapshotCommit({ ...plan, deletedResources }),
    );
    expect(observed).toEqual([{ content: undefined, document: undefined, resource: undefined }]);
  } finally {
    stop();
  }
  expect(useResourceStore.getState().fileSnapshots.doc[saved.path]).toBeUndefined();
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  expect(
    useResourceStore.getState().resources[resourceKey({ id: saved.path, kind: "doc" })],
  ).toBeUndefined();
});

it("moves retained buffers when a rename publication arrives before its command reply", async () => {
  const { input, edit, plan } = setup();
  const path = "docs/Renamed.md";
  await commitPreparedProjectSnapshot(
    prepareProjectSnapshotCommit({
      ...plan,
      index: { ...plan.index, docs: [{ path, kind: "doc", name: "Renamed", revision: 1 }] },
      filePathRemaps: new Map([[saved.path, path]]),
    }),
  );
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  expect(hasPendingDocumentInput(path)).toBe(true);
  expect(
    retainDocumentInput(saved.projectInstanceId, path, "doc-a:markdown", () => {
      throw new Error("lost buffer");
    }),
  ).toBe(input);
  await input.flush();
  expect(edit).toHaveBeenCalledWith(path, [{ op: "set_markdown", markdown: "unsaved text" }], {
    sessionId: "doc-a",
    revision: 1,
  });
});
