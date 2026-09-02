import { useResourceStore } from "@/features/core/resource/resourceStore";
import { lookupGraphResourceByKind } from "@/features/domain/resource/resourceQueries";
import { workbenchDockviewRead } from "@/modules/workbench/public";

export function resolveExecutionGraphPath(targetGraphPath?: string): string | undefined {
  if (targetGraphPath) return targetGraphPath;

  const panel = workbenchDockviewRead.getActiveEditorPanel();
  return panel?.metadata.role === "editor" ? panel.metadata.resourceRef : undefined;
}

export function getExecutionEventTarget(targetGraphPath?: string) {
  const graphPath = resolveExecutionGraphPath(targetGraphPath);
  if (!graphPath) return null;
  const resource = lookupGraphResourceByKind(
    useResourceStore.getState().resources,
    graphPath,
    "event",
  );
  if (!resource?.exists) return null;
  return { graphPath, name: resource.name };
}
