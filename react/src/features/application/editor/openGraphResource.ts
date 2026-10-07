import type { NodeFileKind } from "@/shared/types/domain/resource";
import { lookupNodeFileResource } from "@/features/core/resource/resourceSelectors";
import { useResourceStore } from "@/features/core/resource";
import { openGraphInEditor } from "./openGraphInEditor";

export function resolveGraphResourceMeta(
  path: string,
): { name: string; type: NodeFileKind } | null {
  const resources = useResourceStore.getState().resources;
  const functionMeta = lookupNodeFileResource(resources, path, "function_graph");
  if (functionMeta?.exists) {
    return { name: functionMeta.name, type: "function_graph" };
  }
  const eventMeta = lookupNodeFileResource(resources, path, "event_graph");
  if (eventMeta?.exists) {
    return { name: eventMeta.name, type: "event_graph" };
  }
  return null;
}

export async function openGraphResource(path: string, kind?: NodeFileKind): Promise<void> {
  const meta = kind
    ? (() => {
        const resource = lookupNodeFileResource(useResourceStore.getState().resources, path, kind);
        if (!resource?.exists) return null;
        return { name: resource.name, type: kind };
      })()
    : resolveGraphResourceMeta(path);
  if (!meta) return;
  await openGraphInEditor(path, meta.name, meta.type);
}
