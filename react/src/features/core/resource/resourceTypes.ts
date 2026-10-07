import { toResourceUri, type FileResourceKind } from "@/shared/types/domain/resource";
import type {
  ProjectResourceMeta,
  ResourceKey,
  ResourceRef,
} from "@/features/domain/resource/resourceTypes";

export type {
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
  return toResourceUri(input.kind, input.id);
}

export function buildFileResourceMeta(
  kind: FileResourceKind,
  path: string,
  name: string,
  overrides?: Partial<Omit<ProjectResourceMeta, "id" | "kind" | "name" | "uri">>,
): ProjectResourceMeta {
  return {
    id: path,
    kind,
    name,
    uri: toResourceUri(kind, path),
    exists: true,
    loaded: false,
    hasDirtyDocument: false,
    hasStaleDocument: false,
    hasConflictDocument: false,
    ...overrides,
  };
}
