import { beforeEach, expect, it, vi } from "vitest";
import { createFileActions } from "./createFileActions";
import { createFileProjectionStore } from "@/features/core/resource/fileProjectionStore";
import { createFileTextInput } from "./fileTextInput";
import {
  hasPendingDocumentInput,
  resetDocumentInputs,
  retainDocumentInput,
} from "./documentInputs";
import { startProjectLifecycle } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import {
  buildFileResourceMeta,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
} from "@/features/core/resource";
import type { DocSnapshot, DocEdit, DocCommand } from "@/shared/types/domain/doc";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import { confirmDirtyEditorClose } from "@/features/application/editor/confirmDirtyEditorClose";

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
function setup() {
  const projection = createFileProjectionStore<DocSnapshot>();
  projection.store.getState().install(saved);
  let finish!: (snapshot: DocSnapshot) => void;
  const command = vi.fn(
    (_project: string, operationId: string, _command: DocCommand) =>
      new Promise<{ snapshot: DocSnapshot; mutation: ResourceMutationResultDto }>((resolve) => {
        finish = (snapshot) =>
          resolve({
            snapshot,
            mutation: {
              projectInstanceId: "project-a",
              operationId,
              publicationRevision: snapshot.version.revision,
              moves: [],
              deltas: [],
              projectionReplacements: [],
              projectionStatus: { status: "complete", expectedGraphPaths: [] },
            },
          });
      }),
  );
  const actions = createFileActions<DocSnapshot, DocEdit>(projection, {
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
  useDocumentStateStore.getState().clear();
  useResourceStore.getState().clear();
  useResourceStore.getState().upsertResource(buildFileResourceMeta("doc", saved.path, "Report"));
  markResourceLoaded({ id: saved.path, kind: "doc" });
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
  finish(moved);
  await renaming;
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
