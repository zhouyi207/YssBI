import { beforeEach, expect, it, vi } from "vitest";
import { createFileActions } from "./createFileActions";
import { createFileTextInput } from "./fileTextInput";
import {
  hasPendingDocumentInput,
  releaseDocumentInputs,
  resetDocumentInputs,
  retainDocumentInput,
} from "./documentInputs";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildFileResourceMeta,
  markResourceLoaded,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import type { DocSnapshot, DocEdit, DocCommand } from "@/shared/types/domain/doc";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import { confirmDirtyEditorClose } from "@/features/application/editor/confirmDirtyEditorClose";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

const mocks = vi.hoisted(() => ({ confirm: vi.fn(), settle: vi.fn() }));
vi.mock("@/features/application/editorMutation/projectPublicationCoordinator", () => ({
  projectPublicationCoordinator: {
    capturePublicationRevision: () => 0,
    submit: vi.fn(async () => {}),
  },
}));
vi.mock("@/features/application/editor/saveAllDirtyDocuments", () => ({
  saveAllDirtyDocuments: vi.fn(async () => true),
}));
vi.mock("./resourceActions", () => ({ fileResourceHandlers: { doc: { settle: mocks.settle } } }));
vi.mock("@/features/core/ui/UIStore", () => ({ uiStore: { confirm3: mocks.confirm } }));
vi.mock("@/modules/workbench/public", () => ({
  workbenchLayoutRead: {
    listPanels: () => [
      {
        groupId: "group",
        title: "Report",
        metadata: { role: "editor", resourceKind: "doc", resourceRef: "docs/Report.md" },
      },
    ],
  },
}));

const saved: DocSnapshot = {
  projectInstanceId: "project-a",
  path: "docs/Report.md",
  kind: "doc",
  content: "saved",
  dirty: false,
  version: { sessionId: "doc-a", revision: 0 },
};

function receipt(
  command: DocCommand,
  operationId: string,
  snapshot: DocSnapshot | null,
): ResourceMutationResultDto {
  const creating = command.op === "create" || command.op === "duplicate";
  const source = "path" in command ? command.path : undefined;
  const target = creating || command.op === "rename" ? snapshot!.path : source!;
  const fromRevision = creating ? 0 : command.version.revision;
  const toRevision = snapshot?.version.revision ?? fromRevision + 1;
  const moving = command.op === "rename" && source !== target;
  return {
    projectInstanceId: saved.projectInstanceId,
    operationId,
    publicationRevision: toRevision + 1,
    moves: moving ? [{ kind: "doc", from: source!, to: target, name: command.name }] : [],
    deltas: [
      {
        resource: { kind: "doc", key: target },
        fromRevision,
        toRevision,
        causedBy: operationId,
        payload: moving
          ? { kind: "resource_move", patch: { from: source!, to: target } }
          : {
              kind: "resource_lifecycle",
              patch: {
                before: creating
                  ? null
                  : { kind: "doc", path: target, name: "Report", revision: fromRevision },
                after: snapshot
                  ? { kind: "doc", path: target, name: "Report", revision: toRevision }
                  : null,
              },
            },
      },
    ],
    projectionReplacements: [],
    projectionStatus: { status: "complete", expectedGraphPaths: [] },
  };
}

function setup() {
  useResourceStore.getState().installFileSnapshot(saved);
  let finish!: (snapshot: DocSnapshot) => void;
  const command = vi.fn(
    (_project: string, operationId: string, command: DocCommand) =>
      new Promise<{ snapshot: DocSnapshot; mutation: ResourceMutationResultDto }>((resolve) => {
        finish = (snapshot) =>
          resolve({
            snapshot,
            mutation: receipt(command, operationId, snapshot),
          });
      }),
  );
  const actions = createFileActions<"doc", DocEdit>("doc", {
    read: vi.fn(async () => saved),
    command,
  });
  mocks.settle.mockImplementation(actions.barrier);
  return { actions, command, finish: (snapshot: DocSnapshot) => finish(snapshot) };
}
beforeEach(() => {
  vi.clearAllMocks();
  resetDocumentInputs();
  startProjectLifecycle("project-a");
  mocks.confirm.mockResolvedValue("cancel");
  useResourceStore.getState().clear();
  useResourceStore
    .getState()
    .setSnapshot({ resources: [buildFileResourceMeta("doc", saved.path, "Report")] });
  markResourceLoaded({ id: saved.path, kind: "doc" });
});

it("rejects command snapshots from another project or path before publication", async () => {
  for (const changed of [
    { ...saved, projectInstanceId: "project-b" },
    { ...saved, path: "docs/Other.md" },
  ]) {
    const { actions, command, finish } = setup();
    const observed: DocSnapshot[] = [];
    const stop = useResourceStore.subscribe((state) => {
      observed.push(...Object.values(state.fileSnapshots.doc));
    });
    try {
      const editing = actions.edit(saved.path, [{ op: "set_markdown", markdown: "changed" }]);
      await vi.waitFor(() => expect(command).toHaveBeenCalledOnce());
      finish({ ...changed, version: { ...saved.version, revision: 1 } });
      await expect.soft(editing).rejects.toThrow("Document receipt identity mismatch");
      expect
        .soft(
          observed.every(
            (snapshot) =>
              snapshot.projectInstanceId === saved.projectInstanceId &&
              snapshot.path === saved.path,
          ),
        )
        .toBe(true);
      expect.soft(actions.getSnapshot(saved.path)).toEqual(saved);
      expect.soft(actions.getSnapshot("docs/Other.md")).toBeUndefined();
    } finally {
      stop();
    }
  }
  expect(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
});

it("requires matching lifecycle receipts before installing creation or releasing deletion", async () => {
  let omitDelta = true;
  const created: DocSnapshot = {
    ...saved,
    path: "docs/Allocated.md",
    version: { sessionId: "allocated", revision: 0 },
  };
  const command = vi.fn(async (_project: string, operationId: string, command: DocCommand) => {
    const snapshot = command.op === "delete" ? null : created;
    const mutation = receipt(command, operationId, snapshot);
    if (omitDelta) mutation.deltas = [];
    return { snapshot, mutation };
  });
  const actions = createFileActions<"doc", DocEdit>("doc", {
    read: vi.fn(async () => created),
    command,
  });
  await expect
    .soft(actions.create("Allocated"))
    .rejects.toThrow("Document receipt identity mismatch");
  expect.soft(actions.getSnapshot(created.path)).toBeUndefined();
  expect.soft(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  omitDelta = false;
  await expect(actions.create("Allocated")).resolves.toBe(created.path);
  expect(actions.getSnapshot(created.path)).toEqual(created);
  vi.mocked(projectPublicationCoordinator.submit).mockClear();
  omitDelta = true;
  await expect
    .soft(actions.remove(created.path))
    .rejects.toThrow("Document receipt identity mismatch");
  expect.soft(actions.getSnapshot(created.path)).toEqual(created);
  expect.soft(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
  omitDelta = false;
  await expect(actions.remove(created.path)).resolves.toBeNull();
  expect(actions.getSnapshot(created.path)).toBeUndefined();
});

it("waits for pending edits before deciding whether a window can close", async () => {
  const { actions, command, finish } = setup();
  const editing = actions.edit(saved.path, [{ op: "set_markdown", markdown: "unsaved" }]);
  await vi.waitFor(() => expect(command).toHaveBeenCalledOnce());
  let settled = false;
  const closing = confirmDirtyEditorClose().then((result) => {
    settled = true;
    return result;
  });
  await Promise.resolve();
  expect(settled).toBe(false);
  expect(mocks.confirm).not.toHaveBeenCalled();
  finish({ ...saved, content: "unsaved", dirty: true, version: { ...saved.version, revision: 1 } });
  await editing;
  expect(await closing).toBe(false);
  expect(mocks.confirm).toHaveBeenCalledOnce();
});

it("publishes file content and document flags together on load and release", async () => {
  const key = resourceKey({ id: saved.path, kind: saved.kind });
  const snapshot = {
    ...saved,
    content: "updated",
    dirty: true,
    version: { ...saved.version, revision: 1 },
  };
  const actions = createFileActions<"doc", DocEdit>("doc", {
    read: vi.fn(async () => snapshot),
    command: vi.fn(),
  });
  const observed: unknown[] = [];
  const observe = () => {
    const state = useResourceStore.getState();
    observed.push({
      content: useResourceStore.getState().fileSnapshots.doc[saved.path]?.content,
      dirty: state.documents[key]?.dirty,
      revision: state.resources[key]?.revision,
      summaryDirty: state.resources[key]?.hasDirtyDocument,
    });
  };
  const stopResources = useResourceStore.subscribe(observe);
  try {
    await actions.load(saved.path);
    expect
      .soft(observed)
      .toEqual([{ content: "updated", dirty: true, revision: 1, summaryDirty: true }]);
    observed.length = 0;
    actions.release(saved.path);
    expect(observed).toEqual([
      { content: undefined, dirty: undefined, revision: 1, summaryDirty: false },
    ]);
  } finally {
    stopResources();
  }
});

it("publishes a file receipt once while retaining pending input and missing-file state", async () => {
  const ref = { kind: saved.kind, id: saved.path };
  const key = resourceKey(ref);
  useResourceStore.getState().patchResource(ref, { revision: 0, exists: false });
  useResourceStore.getState().upsertDocument({
    resourceKey: key,
    loaded: false,
    dirty: false,
    stale: true,
    missing: false,
    conflict: true,
  });
  let snapshot: DocSnapshot = { ...saved, dirty: true, version: { ...saved.version, revision: 1 } };
  const actions = createFileActions<"doc", DocEdit>("doc", {
    read: vi.fn(async () => snapshot),
    command: vi.fn(),
  });
  const before = useResourceStore.getState();
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) =>
    observed.push({
      revision: state.resources[key].revision,
      exists: state.resources[key].exists,
      summaryDirty: state.resources[key].hasDirtyDocument,
      ...state.documents[key],
    }),
  );
  try {
    await actions.load(saved.path);
    expect.soft(observed).toEqual([
      {
        revision: 1,
        exists: false,
        summaryDirty: true,
        resourceKey: key,
        loaded: true,
        dirty: true,
        stale: true,
        missing: true,
        conflict: true,
      },
    ]);
    const input = retainDocumentInput(saved.projectInstanceId, saved.path, "doc-a:markdown", () =>
      createFileTextInput(
        snapshot,
        snapshot.content,
        (markdown): DocEdit => ({ op: "set_markdown", markdown }),
        actions,
      ),
    );
    input.change("new unsubmitted input");
    observed.length = 0;
    snapshot = { ...saved, version: { ...saved.version, revision: 2 } };
    await actions.load(saved.path);
    expect(observed).toEqual([
      {
        revision: 2,
        exists: false,
        summaryDirty: true,
        resourceKey: key,
        loaded: true,
        dirty: true,
        stale: true,
        missing: true,
        conflict: true,
      },
    ]);
    expect(input.dirty()).toBe(true);
    expect(before.resources[key].revision).toBe(0);
    expect(before.documents[key].loaded).toBe(false);
  } finally {
    stop();
    resetDocumentInputs();
  }
});

it("retains and submits text entered during rename under the new path and version", async () => {
  const { actions, command, finish } = setup();
  const inputId = "doc-a:markdown";
  const input = retainDocumentInput(saved.projectInstanceId, saved.path, inputId, () =>
    createFileTextInput(
      saved,
      saved.content,
      (markdown): DocEdit => ({ op: "set_markdown", markdown }),
      actions,
    ),
  );
  const renaming = actions.rename(saved.path, "Renamed");
  await vi.waitFor(() => expect(command).toHaveBeenCalledOnce());
  input.change("typed while renaming");
  const moved = { ...saved, path: "docs/Renamed.md", version: { ...saved.version, revision: 1 } };
  const observed: unknown[] = [];
  const stop = useResourceStore.subscribe((state) =>
    observed.push({
      source: state.fileSnapshots.doc[saved.path]?.path,
      target: state.fileSnapshots.doc[moved.path]?.path,
      dirty: state.documents[resourceKey({ kind: moved.kind, id: moved.path })]?.dirty,
    }),
  );
  try {
    finish(moved);
    await renaming;
    expect(observed).toEqual([{ source: undefined, target: moved.path, dirty: true }]);
  } finally {
    stop();
  }
  expect(hasPendingDocumentInput(saved.path)).toBe(false);
  expect(hasPendingDocumentInput(moved.path)).toBe(true);
  expect(
    retainDocumentInput(saved.projectInstanceId, moved.path, inputId, () => {
      throw new Error("lost buffer");
    }),
  ).toBe(input);
  const flushing = input.flush();
  await vi.waitFor(() => expect(command).toHaveBeenCalledTimes(2));
  expect(command.mock.calls[1][2]).toEqual({
    op: "edit",
    path: moved.path,
    version: moved.version,
    edits: [{ op: "set_markdown", markdown: "typed while renaming" }],
  });
  finish({
    ...moved,
    content: "typed while renaming",
    dirty: true,
    version: { ...moved.version, revision: 2 },
  });
  await flushing;
  expect(actions.getSnapshot(moved.path)?.content).toBe("typed while renaming");
  expect(hasPendingDocumentInput(moved.path)).toBe(false);
});

it("keeps file continuations with their original project across input flush and discard", async () => {
  for (const operation of ["rename", "duplicate", "save"] as const) {
    resetDocumentInputs();
    startProjectLifecycle(saved.projectInstanceId);
    useResourceStore.getState().installFileSnapshot(saved);
    let finish!: () => void;
    retainDocumentInput(saved.projectInstanceId, saved.path, "pending", () => ({
      dirty: () => true,
      flush: () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
      prepareDiscard: () => () => {},
    }));
    const command = vi.fn(async () => {
      throw new Error("obsolete command was admitted");
    });
    const actions = createFileActions<"doc", DocEdit>("doc", {
      read: vi.fn(async () => saved),
      command,
    });
    const pending =
      operation === "rename"
        ? actions.rename(saved.path, "Renamed")
        : actions[operation](saved.path);
    const settled = Promise.allSettled([pending]);
    startProjectLifecycle(saved.projectInstanceId);
    finish();
    expect.soft((await settled)[0]).toMatchObject({
      status: "rejected",
      reason: { code: "stale_project_lifecycle" },
    });
    expect.soft(command).not.toHaveBeenCalled();
  }

  for (const missing of [false, true]) {
    resetDocumentInputs();
    startProjectLifecycle(saved.projectInstanceId);
    useResourceStore.getState().clear();
    useResourceStore.getState().setSnapshot({
      resources: [buildFileResourceMeta(saved.kind, saved.path, "Report", { exists: !missing })],
    });
    useResourceStore.getState().installFileSnapshot(saved);
    const actions = createFileActions<"doc", DocEdit>("doc", {
      read: vi.fn(async () => saved),
      command: vi.fn(async (_project, operationId, command) => {
        const snapshot = { ...saved, version: { ...saved.version, revision: 1 } };
        return { snapshot, mutation: receipt(command, operationId, snapshot) };
      }),
    });
    const input = retainDocumentInput(saved.projectInstanceId, saved.path, "markdown", () =>
      createFileTextInput(
        saved,
        saved.content,
        (markdown): DocEdit => ({ op: "set_markdown", markdown }),
        actions,
      ),
    );
    input.change("unfinished text");
    const successor = {
      ...saved,
      projectInstanceId: "project-b",
      dirty: !missing,
      content: "successor",
      version: { sessionId: "successor", revision: 0 },
    };
    const stop = input.subscribe(() => {
      startProjectLifecycle(successor.projectInstanceId);
      useResourceStore.getState().clear();
      useResourceStore.getState().installFileSnapshot(successor);
    });
    try {
      const [outcome] = await Promise.allSettled([actions.discard(saved.path)]);
      expect.soft(outcome).toMatchObject({
        status: "rejected",
        reason: { code: "stale_project_lifecycle" },
      });
      expect.soft(actions.getSnapshot(saved.path)).toBe(successor);
      expect
        .soft(
          useResourceStore.getState().documents[resourceKey({ id: saved.path, kind: saved.kind })]
            ?.dirty,
        )
        .toBe(successor.dirty);
    } finally {
      stop();
    }
  }
});

it("preserves replacement input registrations when rename notification reenters their owner", async () => {
  const moved = { ...saved, path: "docs/Renamed.md", version: { ...saved.version, revision: 1 } };
  const inputId = "doc-a:markdown";
  for (const replaceProject of [true, false]) {
    resetDocumentInputs();
    startProjectLifecycle(saved.projectInstanceId);
    useResourceStore.getState().clear();
    vi.mocked(projectPublicationCoordinator.submit).mockClear();
    const { actions, command, finish } = setup();
    const acquire = (snapshot: DocSnapshot) =>
      retainDocumentInput(snapshot.projectInstanceId, snapshot.path, inputId, () =>
        createFileTextInput(
          snapshot,
          snapshot.content,
          (markdown): DocEdit => ({ op: "set_markdown", markdown }),
          actions,
        ),
      );
    const input = acquire(saved);
    const renaming = actions.rename(saved.path, "Renamed");
    const settled = Promise.allSettled([renaming]);
    await vi.waitFor(() => expect(command).toHaveBeenCalledOnce());
    const projectInstanceId = replaceProject ? "project-b" : saved.projectInstanceId;
    const nextSource = { ...saved, projectInstanceId, content: "new source" };
    const nextTarget = { ...moved, projectInstanceId, content: "new target" };
    let sourceInput: ReturnType<typeof acquire> | undefined;
    let targetInput: ReturnType<typeof acquire> | undefined;
    const stop = input.subscribe(() => {
      releaseDocumentInputs(saved.path);
      releaseDocumentInputs(moved.path);
      if (replaceProject) {
        startProjectLifecycle(projectInstanceId);
        useResourceStore.getState().clear();
        useResourceStore.getState().installFileSnapshot(nextSource);
        useResourceStore.getState().installFileSnapshot(nextTarget);
      }
      sourceInput = acquire(nextSource);
      targetInput = acquire(nextTarget);
    });
    try {
      finish(moved);
      const [outcome] = await settled;
      expect.soft(acquire(nextSource)).toBe(sourceInput);
      expect.soft(acquire(nextTarget)).toBe(targetInput);
      expect.soft(targetInput?.getValue()).toBe("new target");
      if (replaceProject) {
        expect
          .soft(outcome)
          .toMatchObject({ status: "rejected", reason: { code: "stale_project_lifecycle" } });
        expect.soft(actions.getSnapshot(saved.path)).toBe(nextSource);
        expect.soft(actions.getSnapshot(moved.path)).toBe(nextTarget);
        expect.soft(projectPublicationCoordinator.submit).not.toHaveBeenCalled();
      } else {
        expect.soft(outcome).toEqual({ status: "fulfilled", value: moved });
        expect.soft(projectPublicationCoordinator.submit).toHaveBeenCalledOnce();
      }
    } finally {
      stop();
    }
  }
});
