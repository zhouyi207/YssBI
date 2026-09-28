import { useMemo } from "react";
import {
  lookupNodeFileResource as lookupDomainGraphResource,
  lookupNodeFileResourceByKind,
} from "@/features/domain/resource/resourceQueries";
import { useResourceStore } from "./resourceStore";
import type { ProjectResourceMeta, ResourceKey } from "@/features/domain/resource/resourceTypes";

export type GraphResourceRecord = Record<string, Pick<ProjectResourceMeta, "id" | "name">>;

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

export function selectGraphResourcesByKind(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  kind: "event_graph" | "function_graph",
): GraphResourceRecord {
  const result: GraphResourceRecord = {};
  for (const resource of Object.values(resources)) {
    if (resource.kind !== kind || !resource.exists) continue;
    result[resource.id] = {
      id: resource.id,
      name: resource.name,
    };
  }
  return result;
}

export function useGraphResourcesByKind(
  kind: "event_graph" | "function_graph",
): GraphResourceRecord {
  const resources = useResourceStore((state) => state.resources);
  return useMemo(() => selectGraphResourcesByKind(resources, kind), [resources, kind]);
}
