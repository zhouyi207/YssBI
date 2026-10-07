import { toResourceUri } from "@/shared/types/domain/resource";
import { type NodeFileKind } from "@/shared/types/domain/resource";
import type { ProjectResourceMeta, ResourceKey } from "./resourceTypes";

export function lookupNodeFileResource(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
): ProjectResourceMeta | undefined {
  return (
    resources[toResourceUri("event_graph", graphPath)] ??
    resources[toResourceUri("function_graph", graphPath)]
  );
}

export function lookupNodeFileResourceByKind(
  resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>,
  graphPath: string,
  kind: NodeFileKind,
): ProjectResourceMeta | undefined {
  return resources[toResourceUri(kind, graphPath)];
}
