import { workbenchLayoutRead } from "@/modules/workbench/public";
import { isResourceDocumentDirty, resourceKey } from "@/features/core/resource";
import type { FileResourceKind } from "@/shared/types/domain/resource";
import { resolveResourceDisplayName } from "./resolveResourceDisplayName";

interface EditorFileSnapshot {
  /** FlexLayout group that owns the editor panel. */
  groupId: string;
  /** Opaque file resource reference. */
  resourceRef: string;
  resourceKind: FileResourceKind;
  /** Display title for prompts. */
  title: string;
}

/** Collect each open file once, even when it has multiple panels. */
export function collectEditorFiles(): EditorFileSnapshot[] {
  const seen = new Set<string>();
  const files: EditorFileSnapshot[] = [];
  for (const panel of workbenchLayoutRead.listPanels()) {
    if (panel.metadata.role !== "editor") continue;
    const { resourceKind, resourceRef } = panel.metadata;
    if (resourceKind === "database") continue;
    const key = resourceKey({ id: resourceRef, kind: resourceKind });
    if (seen.has(key)) continue;
    seen.add(key);
    files.push({
      groupId: panel.groupId,
      resourceRef,
      resourceKind,
      title: resolveResourceDisplayName(
        { id: resourceRef, kind: resourceKind },
        panel.title ?? resourceRef,
      ),
    });
  }
  return files;
}

export function collectDirtyEditorPanels(): EditorFileSnapshot[] {
  return collectEditorFiles().filter((file) =>
    isResourceDocumentDirty({ id: file.resourceRef, kind: file.resourceKind }),
  );
}
