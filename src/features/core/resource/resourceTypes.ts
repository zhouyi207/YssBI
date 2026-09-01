import {
  toGraphResourceUri,
  type GraphResourceKind,
} from "@/shared/types/domain/graphResourcePath";
import type {
  ProjectResourceMeta,
  ResourceKey,
  ResourceRef,
} from "@/features/domain/resource/resourceTypes";

export type {
  BackendProjectResourceMeta,
  ProjectResourceMeta,
  ResourceKey,
  ResourceKind,
  ResourceRef,
} from "@/features/domain/resource/resourceTypes";

type ResourceKeyInput = ResourceRef | Pick<ProjectResourceMeta, "kind" | "id" | "uri">;

export function resourceKey(input: ResourceKeyInput): ResourceKey {
  if ("uri" in input && input.uri) {
    return input.uri;
  }
  return resourceKeyFromRef(input as ResourceRef);
}

function resourceKeyFromRef(ref: ResourceRef): ResourceKey {
  switch (ref.kind) {
    case "event":
    case "function":
      return toGraphResourceUri(ref.kind, ref.id);
    case "chart":
      return `yssbi://chart/${ref.id}`;
    case "database":
      return `yssbi://database/${ref.id}`;
    case "variable":
      return `yssbi://variable/${ref.id}`;
  }
}

export function buildGraphResourceMeta(
  kind: GraphResourceKind,
  path: string,
  name: string,
  overrides?: Partial<Omit<ProjectResourceMeta, "id" | "kind" | "name" | "uri">>,
): ProjectResourceMeta {
  return {
    id: path,
    kind,
    name,
    uri: toGraphResourceUri(kind, path),
    exists: true,
    loaded: false,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
    ...overrides,
  };
}
