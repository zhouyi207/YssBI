import {
  lookupNodeFileResource as lookupDomainGraphResource,
  lookupNodeFileResourceByKind,
} from "@/features/domain/resource/resourceQueries";
import { useResourceStore } from "./resourceStore";
import type { ProjectResourceMeta, ResourceKey } from "@/features/domain/resource/resourceTypes";

export function lookupNodeFileResource(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  graphPath: string,
  kind?: "event_graph" | "function_graph",
): ProjectResourceMeta | null {
  return (
    (kind
      ? lookupNodeFileResourceByKind(resources, graphPath, kind)
      : lookupDomainGraphResource(resources, graphPath)) ?? null
  );
}

export function lookupNodeFileKind(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
): "event_graph" | "function_graph" | undefined {
  if (lookupNodeFileResourceByKind(resources, graphPath, "event_graph")?.exists)
    return "event_graph";
  if (lookupNodeFileResourceByKind(resources, graphPath, "function_graph")?.exists)
    return "function_graph";
  return undefined;
}

export function getNodeFileKind(graphPath: string): "event_graph" | "function_graph" | undefined {
  return lookupNodeFileKind(useResourceStore.getState().resources, graphPath);
}
