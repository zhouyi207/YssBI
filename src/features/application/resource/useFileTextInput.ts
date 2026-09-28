import { useEffect, useState, useSyncExternalStore } from "react";
import type { FileSnapshot } from "@/shared/types/domain/fileDocument";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import { retainDocumentInput } from "./documentInputs";
import { createFileTextInput, type FileTextInputActions } from "./fileTextInput";

export function useFileTextInput<S extends FileSnapshot<FileResourceKind, unknown>, E>(
  snapshot: S,
  initial: string,
  makeEdit: (text: string) => E,
  actions: FileTextInputActions<S, E>,
  inputId: string,
) {
  const [input] = useState(() =>
    retainDocumentInput(
      snapshot.projectInstanceId,
      snapshot.path,
      `${snapshot.version.sessionId}:${inputId}`,
      () => createFileTextInput(snapshot, initial, makeEdit, actions),
    ),
  );
  const value = useSyncExternalStore(input.subscribe, input.getValue, input.getValue);
  useEffect(() => {
    input.sync(initial, snapshot.version, makeEdit);
  }, [input, initial, snapshot.version, makeEdit]);
  return { value, change: input.change, flush: input.flush };
}
