import { collectEditorFiles } from "./editorPanelDirty";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import {
  fileResourceHandlers,
  type FileResourceRef,
} from "@/features/application/resource/resourceActions";

export async function settleEditorFileEdits(files?: readonly FileResourceRef[]): Promise<void> {
  const refs =
    files ??
    collectEditorFiles().map((file) => ({ id: file.resourceRef, kind: file.resourceKind }));
  const distinct = new Map(refs.map((ref) => [resourceKey(ref), ref]));
  await Promise.all(
    [...distinct.values()].map((ref) => fileResourceHandlers[ref.kind].settle(ref.id)),
  );
}
