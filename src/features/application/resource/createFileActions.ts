import type { FileVersion, FileCommand } from "@/shared/types/domain/fileDocument";
import type { ResourceMutationResultDto } from "@/shared/types/domain/editorMutation";
import {
  markResourceDirty,
  useResourceStore,
  resourceKey,
  type FileSnapshotKind,
  type FileSnapshotByKind,
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

export function createFileActions<K extends FileSnapshotKind, E>(
  kind: K,
  service: {
    read(project: string, path: string): Promise<FileSnapshotByKind[K]>;
    command(
      project: string,
      operation: string,
      command: FileCommand<E>,
    ): Promise<{ snapshot: FileSnapshotByKind[K] | null; mutation: ResourceMutationResultDto }>;
  },
) {
  type S = FileSnapshotByKind[K];
  const queues = new Map<string, Promise<unknown>>();
  const reads = new Map<string, { isCurrent(): boolean; promise: Promise<S> }>();
  const getSnapshot = (path: string): S | undefined =>
    useResourceStore.getState().fileSnapshots[kind][path];
  function releaseDocumentProjection(path: string): void {
    releaseDocumentInputs(path);
    useResourceStore.getState().removeFileSnapshot(kind, path);
  }
  function install(snapshot: S, renamedFrom?: string): void {
    useResourceStore.getState().installFileSnapshot(snapshot, {
      pendingInput: hasPendingDocumentInput(snapshot.path),
      renamedFrom,
    });
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
    const isCurrentRead = useResourceStore.getState().beginFileRead(kind, path);
    const promise = service
      .read(context.projectInstanceId, path)
      .then((snapshot) => {
        context.assertCurrent();
        if (snapshot.projectInstanceId !== context.projectInstanceId || snapshot.path !== path)
          throw new Error("Document identity mismatch");
        if (!isCurrentRead()) {
          const current = getSnapshot(path);
          if (current?.projectInstanceId === context.projectInstanceId) return current;
          throw new Error("Document read lifecycle ended");
        }
        install(snapshot);
        return getSnapshot(path)!;
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
      result.mutation.operationId !== context.operationId ||
      !matchesCommandReceipt(command, result.snapshot, result.mutation)
    )
      throw new Error("Document receipt identity mismatch");
    let renamedFrom: string | undefined;
    if (command.op === "rename" && result.snapshot && result.snapshot.path !== command.path) {
      renamedFrom = command.path;
      remapDocumentInputs(
        command.path,
        result.snapshot.path,
        command.version,
        result.snapshot.version,
      );
    }
    if (result.snapshot) install(result.snapshot, renamedFrom);
    if (command.op === "delete") {
      releaseDocumentProjection(command.path);
    }
    await projectPublicationCoordinator.submit({ result: result.mutation });
    context.assertCurrent();
    return result.snapshot;
  }

  function matchesCommandReceipt(
    command: FileCommand<E>,
    snapshot: S | null,
    mutation: ResourceMutationResultDto,
  ): boolean {
    const creating = command.op === "create" || command.op === "duplicate";
    const move =
      command.op === "rename"
        ? mutation.moves.find((entry) => entry.kind === kind && entry.from === command.path)
        : undefined;
    const path = creating ? snapshot?.path : (move?.to ?? command.path);
    const delta = mutation.deltas.find(
      (entry) =>
        entry.resource.kind === kind &&
        entry.resource.key === path &&
        entry.causedBy === mutation.operationId,
    );
    if (!delta) return false;
    if (
      snapshot &&
      (snapshot.projectInstanceId !== mutation.projectInstanceId ||
        snapshot.kind !== kind ||
        snapshot.path !== path ||
        snapshot.version.revision !== delta.toRevision)
    )
      return false;
    const payload = delta.payload;
    if (creating) {
      return (
        snapshot !== null &&
        (command.op === "create" || snapshot.path !== command.path) &&
        payload.kind === "resource_lifecycle" &&
        payload.patch.before === null &&
        payload.patch.after !== null
      );
    }
    if (
      delta.fromRevision !== command.version.revision ||
      (snapshot && snapshot.version.sessionId !== command.version.sessionId)
    )
      return false;
    if (!snapshot) {
      return (
        (command.op === "delete" || command.op === "discard") &&
        payload.kind === "resource_lifecycle" &&
        payload.patch.before !== null &&
        payload.patch.after === null
      );
    }
    if (command.op === "delete") return false;
    if (command.op === "rename" && path !== command.path) {
      return (
        payload.kind === "resource_move" &&
        payload.patch.from === command.path &&
        payload.patch.to === path
      );
    }
    return (
      payload.kind === "resource_lifecycle" &&
      payload.patch.before !== null &&
      payload.patch.after !== null
    );
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
    const task = (queues.get(key) ?? Promise.resolve())
      .catch(() => {})
      .then(async () => {
        context.assertCurrent();
        let current = getSnapshot(path);
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
    const context = captureProjectCommandContext();
    await flushDocumentInputs(path);
    context.assertCurrent();
    return enqueue(path, (version) => ({ op: "rename", path, version, name }));
  }
  async function duplicateDocument(path: string) {
    const context = captureProjectCommandContext();
    await flushDocumentInputs(path);
    context.assertCurrent();
    return enqueue(path, (version) => ({ op: "duplicate", path, version }));
  }
  const deleteDocument = (path: string) =>
    enqueue(path, (version) => ({ op: "delete", path, version }));
  async function discardDocument(path: string, expected?: FileVersion) {
    const discardInputs = prepareDocumentInputDiscard(path);
    const context = captureProjectCommandContext();
    await documentQueueBarrier(path);
    context.assertCurrent();
    const current = getSnapshot(path);
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
    context.assertCurrent();
    discardInputs();
    context.assertCurrent();
    if (result)
      markResourceDirty(
        { id: path, kind: result.kind },
        result.dirty || hasPendingDocumentInput(path),
      );
    else if (!hasPendingDocumentInput(path)) releaseDocumentProjection(path);
    context.assertCurrent();
    return result;
  }
  async function saveDocument(path: string): Promise<boolean> {
    const context = captureProjectCommandContext();
    await flushDocumentInputs(path);
    context.assertCurrent();
    const snapshot = await enqueue(path, (version) => ({ op: "save", path, version }));
    context.assertCurrent();
    return (
      snapshot?.dirty === false &&
      !hasPendingDocumentInput(path) &&
      getSnapshot(path)?.dirty === false
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
    getSnapshot,
  };
}
