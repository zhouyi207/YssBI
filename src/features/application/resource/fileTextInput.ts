import { createStore } from "zustand/vanilla";
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
  const { projectInstanceId, kind } = snapshot;
  let path = snapshot.path;
  const store = createStore(() => ({
    text: "",
    dirty: false,
    generation: 0,
    version: snapshot.version,
  }));
  store.setState({ text: initial });
  let latestText = initial;
  let editor = makeEdit;
  let inFlight: Promise<void> | null = null;
  let released = false;
  const isCurrent = () =>
    !released &&
    owner.projectInstanceId === projectInstanceId &&
    isProjectLifecycleStateCurrent(owner);

  return {
    getValue: () => store.getState().text,
    subscribe: store.subscribe,
    sync(text: string, version: FileVersion, nextEditor: (text: string) => E) {
      latestText = text;
      editor = nextEditor;
      if (store.getState().dirty || !isCurrent()) return;
      store.setState({ text, version });
    },
    dirty: () => store.getState().dirty,
    remap(nextPath: string, fromVersion?: FileVersion, toVersion?: FileVersion) {
      if (!isCurrent()) return;
      path = nextPath;
      const { version } = store.getState();
      // Only a rename of the buffer's exact base can safely advance its edit version.
      if (
        fromVersion &&
        toVersion &&
        version.sessionId === fromVersion.sessionId &&
        version.revision === fromVersion.revision &&
        toVersion.sessionId === fromVersion.sessionId &&
        toVersion.revision === fromVersion.revision + 1
      )
        store.setState({ version: toVersion });
    },
    change(text: string) {
      if (!isCurrent()) return;
      const buffer = store.getState();
      store.setState({
        text,
        dirty: true,
        generation: buffer.generation + 1,
        version: buffer.dirty
          ? buffer.version
          : (actions.getSnapshot(path)?.version ?? buffer.version),
      });
      if (!isCurrent()) return;
      markResourceDirty({ id: path, kind }, true);
    },
    async flush() {
      // Another waiter may start the next edit before this flush resumes.
      while (inFlight) await inFlight;
      const buffer = store.getState();
      if (!buffer.dirty || !isCurrent()) return;
      const submitted = buffer;
      const task = (async () => {
        const result = await actions.edit(path, [editor(submitted.text)], submitted.version);
        if (!result) throw new Error("Document edit returned no snapshot");
        if (!isCurrent()) return;
        store.setState((current) => ({
          version: result.version,
          dirty: current.generation === submitted.generation ? false : current.dirty,
        }));
        if (!isCurrent()) return;
        markResourceDirty({ id: path, kind }, result.dirty || hasPendingDocumentInput(path));
      })();
      inFlight = task;
      try {
        await task;
      } finally {
        if (inFlight === task) inFlight = null;
      }
    },
    prepareDiscard() {
      const generation = store.getState().generation;
      return () => {
        if (store.getState().generation !== generation || !isCurrent()) return;
        store.setState({ dirty: false, generation: generation + 1, text: latestText });
      };
    },
    release() {
      released = true;
    },
  };
}
