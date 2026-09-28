import { useResourceStore } from "@/features/core/resource/resourceStore";
import { lookupNodeFileResourceByKind } from "@/features/domain/resource/resourceQueries";
import { workbenchLayoutRead } from "@/modules/workbench/public";

export function resolveExecutionGraphPath(targetGraphPath?: string): string | undefined {
  if (targetGraphPath) return targetGraphPath;

  const panel = workbenchLayoutRead.getActiveEditorPanel();
  return panel?.metadata.role === "editor" ? panel.metadata.resourceRef : undefined;
}

export function getExecutionEventTarget(targetGraphPath?: string) {
  const graphPath = resolveExecutionGraphPath(targetGraphPath);
  if (!graphPath) return null;
  const resource = lookupNodeFileResourceByKind(
    useResourceStore.getState().resources,
    graphPath,
    "event_graph",
  );
  if (!resource?.exists) return null;
  return { graphPath, name: resource.name };
}
