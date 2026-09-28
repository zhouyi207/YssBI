import type { FileSnapshot, FileVersion } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import { markResourceDirty } from "@/features/core/resource";
import {
  captureProjectLifecycleState,
  isProjectLifecycleStateCurrent,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { hasPendingDocumentInput } from "./documentInputs";

export interface FileTextInputActions<S, E> {
  edit(path: string, edits: E[], version?: FileVersion): Promise<S | null>;
  getSnapshot(path: string): S | undefined;
}

export function createFileTextInput<S extends FileSnapshot<FileResourceKind, unknown>, E>(
  snapshot: S,
  initial: string,
  makeEdit: (text: string) => E,
  actions: FileTextInputActions<S, E>,
) {
  const owner = captureProjectLifecycleState();
  let path = snapshot.path;
  const buffer = { text: initial, dirty: false, generation: 0, version: snapshot.version };
  const listeners = new Set<() => void>();
  let latestText = initial;
  let editor = makeEdit;
  let inFlight: Promise<void> | null = null;
  let released = false;
  const isCurrent = () =>
    !released &&
    owner.projectInstanceId === snapshot.projectInstanceId &&
    isProjectLifecycleStateCurrent(owner);
  const publish = () => {
    for (const listener of listeners) listener();
  };

  return {
    getValue: () => buffer.text,
    subscribe(listener: () => void) {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    sync(text: string, version: FileVersion, nextEditor: (text: string) => E) {
      latestText = text;
      editor = nextEditor;
      if (buffer.dirty || !isCurrent()) return;
      const changed = buffer.text !== text;
      buffer.text = text;
      buffer.version = version;
      if (changed) publish();
    },
    dirty: () => buffer.dirty,
    remap(nextPath: string, fromVersion?: FileVersion, toVersion?: FileVersion) {
      if (!isCurrent()) return;
      path = nextPath;
      // Only a rename of the buffer's exact base can safely advance its edit version.
      if (
        fromVersion &&
        toVersion &&
        buffer.version.sessionId === fromVersion.sessionId &&
        buffer.version.revision === fromVersion.revision &&
        toVersion.sessionId === fromVersion.sessionId &&
        toVersion.revision === fromVersion.revision + 1
      )
        buffer.version = toVersion;
    },
    change(text: string) {
      if (!isCurrent()) return;
      if (!buffer.dirty) buffer.version = actions.getSnapshot(path)?.version ?? buffer.version;
      buffer.text = text;
      buffer.dirty = true;
      buffer.generation++;
      markResourceDirty({ id: path, kind: snapshot.kind }, true);
      publish();
    },
    async flush() {
      if (inFlight) await inFlight;
      if (!buffer.dirty || !isCurrent()) return;
      const submitted = { ...buffer };
      const task = (async () => {
        const result = await actions.edit(path, [editor(submitted.text)], submitted.version);
        if (!result) throw new Error("Document edit returned no snapshot");
        if (!isCurrent()) return;
        buffer.version = result.version;
        if (buffer.generation === submitted.generation) buffer.dirty = false;
        markResourceDirty(
          { id: path, kind: snapshot.kind },
          result.dirty || hasPendingDocumentInput(path),
        );
      })();
      inFlight = task;
      try {
        await task;
      } finally {
        if (inFlight === task) inFlight = null;
      }
    },
    prepareDiscard() {
      const generation = buffer.generation;
      return () => {
        if (buffer.generation !== generation || !isCurrent()) return;
        buffer.dirty = false;
        buffer.generation++;
        buffer.text = latestText;
        publish();
      };
    },
    release() {
      released = true;
    },
  };
}
