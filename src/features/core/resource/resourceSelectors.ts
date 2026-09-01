import { useMemo } from "react";
import {
  lookupGraphResource as lookupDomainGraphResource,
  lookupGraphResourceByKind,
} from "@/features/domain/resource/resourceQueries";
import { useResourceStore } from "./resourceStore";
import type { ProjectResourceMeta, ResourceKey } from "@/features/domain/resource/resourceTypes";

export type GraphResourceRecord = Record<string, Pick<ProjectResourceMeta, "id" | "name">>;

export function lookupGraphResource(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  graphPath: string,
  kind?: "event" | "function",
): ProjectResourceMeta | null {
  return (
    (kind
      ? lookupGraphResourceByKind(resources, graphPath, kind)
      : lookupDomainGraphResource(resources, graphPath)) ?? null
  );
}

export function lookupGraphResourceKind(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
): "event" | "function" | undefined {
  if (lookupGraphResourceByKind(resources, graphPath, "event")?.exists) return "event";
  if (lookupGraphResourceByKind(resources, graphPath, "function")?.exists) return "function";
  return undefined;
}

export function selectGraphResourcesByKind(
  resources: Record<ResourceKey, ProjectResourceMeta>,
  kind: "event" | "function",
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

export function useGraphResourcesByKind(kind: "event" | "function"): GraphResourceRecord {
  const resources = useResourceStore((state) => state.resources);
  return useMemo(() => selectGraphResourcesByKind(resources, kind), [resources, kind]);
}
