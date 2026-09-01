import type { EditorPanelMetadata } from "@/modules/workbench/public";
import { useResourceRead } from "@/features/core/resource/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";

export function useEditorPanelDirty(
  metadata: Pick<EditorPanelMetadata, "resourceRef" | "resourceKind"> | null,
): boolean {
  const documentKey = metadata
    ? resourceKey({ id: metadata.resourceRef, kind: metadata.resourceKind })
    : null;
  return useResourceRead((snapshot) =>
    documentKey ? snapshot.documents[documentKey]?.dirty === true : false,
  );
}
