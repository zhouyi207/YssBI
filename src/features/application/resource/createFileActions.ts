import type { FileSnapshot, FileVersion, FileCommand } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import type { createFileProjectionStore } from "@/features/core/resource/fileProjectionStore";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import {
  markResourceDirty,
  markResourceLoaded,
  clearResourceDocumentState,
  useResourceStore,
  resourceKey,
} from "@/features/core/resource";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  prepareDocumentInputDiscard,
  flushDocumentInputs,
  hasPendingDocumentInput,
  releaseDocumentInputs,
  remapDocumentInputs,
} from "./documentInputs";

export function createFileActions<S extends FileSnapshot<FileResourceKind, unknown>, E>(
  projection: ReturnType<typeof createFileProjectionStore<S>>,
  service: {
    read(project: string, path: string): Promise<S>;
    command(
      project: string,
      operation: string,
      command: FileCommand<E>,
    ): Promise<{ snapshot: S | null; mutation: ResourceMutationResultDto }>;
  },
) {
  const queues = new Map<string, Promise<unknown>>();
  const reads = new Map<string, { isCurrent(): boolean; promise: Promise<S> }>();
  function releaseDocumentProjection(path: string): void {
    releaseDocumentInputs(path);
    removeProjection(path);
  }
  function removeProjection(path: string): void {
    const previous = projection.store.getState().documents[path];
    if (previous) clearResourceDocumentState({ id: path, kind: previous.kind });
    projection.store.getState().remove(path);
  }
  function install(snapshot: S): void {
    if (!projection.store.getState().install(snapshot)) return;
    const ref = { kind: snapshot.kind, id: snapshot.path };
    markResourceLoaded(ref);
    markResourceDirty(ref, snapshot.dirty || hasPendingDocumentInput(snapshot.path));
    useResourceStore.getState().patchResource(ref, { revision: snapshot.version.revision });
  }
  async function loadDocument(path: string): Promise<S> {
    const context = captureProjectCommandContext();
    const key = JSON.stringify([
      context.projectInstanceId,
      context.projectEpoch,
      context.publicationRevision,
      path,
    ]);
    const pending = reads.get(key);
    if (pending?.isCurrent()) return pending.promise;
    const isCurrentRead = projection.beginRead(path);
    const promise = service
      .read(context.projectInstanceId, path)
      .then((snapshot) => {
        context.assertCurrent();
        if (snapshot.projectInstanceId !== context.projectInstanceId || snapshot.path !== path)
          throw new Error("Document identity mismatch");
        if (!isCurrentRead()) {
          const current = projection.store.getState().documents[path];
          if (current?.projectInstanceId === context.projectInstanceId) return current;
          throw new Error("Document read lifecycle ended");
        }
        install(snapshot);
        return projection.store.getState().documents[path];
      })
      .finally(() => {
        if (reads.get(key)?.promise === promise) reads.delete(key);
      });
    reads.set(key, { isCurrent: isCurrentRead, promise });
    return promise;
  }

  async function submit(
    command: FileCommand<E>,
    context: ReturnType<typeof captureProjectCommandContext>,
  ) {
    const result = await service.command(context.projectInstanceId, context.operationId, command);
    context.assertCurrent();
    if (
      result.mutation.projectInstanceId !== context.projectInstanceId ||
      result.mutation.operationId !== context.operationId
    )
      throw new Error("Document receipt identity mismatch");
    if (command.op === "rename" && result.snapshot && result.snapshot.path !== command.path) {
      remapDocumentInputs(
        command.path,
        result.snapshot.path,
        command.version,
        result.snapshot.version,
      );
      removeProjection(command.path);
    }
    if (result.snapshot) install(result.snapshot);
    if (command.op === "delete") {
      releaseDocumentProjection(command.path);
    }
    await projectPublicationCoordinator.submit({ result: result.mutation });
    context.assertCurrent();
    return result.snapshot;
  }

  async function documentQueueBarrier(path: string): Promise<unknown> {
    const context = captureProjectCommandContext();
    return queues.get(`${context.projectInstanceId}:${path}`) ?? Promise.resolve();
  }

  function enqueue(
    path: string,
    build: (version: FileVersion) => FileCommand<E>,
  ): Promise<S | null> {
    const context = captureProjectCommandContext();
    const key = `${context.projectInstanceId}:${path}`;
    const store = projection.store;
    const task = (queues.get(key) ?? Promise.resolve())
      .catch(() => {})
      .then(async () => {
        context.assertCurrent();
        let current = store.getState().documents[path];
        if (!current || current.projectInstanceId !== context.projectInstanceId)
          current = await loadDocument(path);
        context.assertCurrent();
        try {
          return await submit(build(current.version), context);
        } catch (error) {
          if (context.isCurrent()) await loadDocument(path).catch(() => {});
          throw error;
        }
      })
      .finally(() => {
        if (queues.get(key) === task) queues.delete(key);
      });
    queues.set(key, task);
    return task;
  }

  const editDocument = (path: string, edits: E[], expected?: FileVersion) =>
    enqueue(path, (version) => ({ op: "edit", path, version: expected ?? version, edits }));
  async function renameDocument(path: string, name: string) {
    await flushDocumentInputs(path);
    return enqueue(path, (version) => ({ op: "rename", path, version, name }));
  }
  async function duplicateDocument(path: string) {
    await flushDocumentInputs(path);
    return enqueue(path, (version) => ({ op: "duplicate", path, version }));
  }
  const deleteDocument = (path: string) =>
    enqueue(path, (version) => ({ op: "delete", path, version }));
  async function discardDocument(path: string, expected?: FileVersion) {
    const discardInputs = prepareDocumentInputDiscard(path);
    const context = captureProjectCommandContext();
    await documentQueueBarrier(path);
    context.assertCurrent();
    const current = projection.store.getState().documents[path];
    const missingClean =
      current &&
      !current.dirty &&
      useResourceStore.getState().resources[resourceKey({ id: path, kind: current.kind })]
        ?.exists === false;
    if (
      missingClean &&
      expected &&
      (expected.sessionId !== current.version.sessionId ||
        expected.revision !== current.version.revision)
    )
      throw new Error("Document discard version changed");
    const result = missingClean
      ? null
      : await enqueue(path, (version) => ({
          op: "discard",
          path,
          version: expected ?? version,
        }));
    discardInputs();
    if (result)
      markResourceDirty(
        { id: path, kind: result.kind },
        result.dirty || hasPendingDocumentInput(path),
      );
    else if (!hasPendingDocumentInput(path)) releaseDocumentProjection(path);
    return result;
  }
  async function saveDocument(path: string): Promise<boolean> {
    await flushDocumentInputs(path);
    const snapshot = await enqueue(path, (version) => ({ op: "save", path, version }));
    return (
      snapshot?.dirty === false &&
      !hasPendingDocumentInput(path) &&
      projection.store.getState().documents[path]?.dirty === false
    );
  }
  async function createDocument(name: string): Promise<string> {
    const snapshot = await submit({ op: "create", name }, captureProjectCommandContext());
    if (!snapshot) throw new Error("Missing created document");
    return snapshot.path;
  }

  return {
    load: loadDocument,
    release: releaseDocumentProjection,
    edit: editDocument,
    rename: renameDocument,
    duplicate: duplicateDocument,
    remove: deleteDocument,
    discard: discardDocument,
    save: saveDocument,
    create: createDocument,
    barrier: documentQueueBarrier,
    getSnapshot: (path: string) => projection.store.getState().documents[path],
  };
}
