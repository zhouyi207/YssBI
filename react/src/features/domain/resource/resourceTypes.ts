import type { ResourceKind } from "@/shared/types/domain/resource";
export type { ResourceKind, ResourceRef } from "@/shared/types/domain/resource";

/** Canonical store key — always equals `ProjectResourceMeta.uri`. */
export type ResourceKey = string;

export interface ProjectResourceMeta {
  id: string;
  kind: ResourceKind;
  name: string;
  uri: string;
  /** Opaque backend path for database creation descriptors. */
  resourcePath?: string;
  revision?: number;
  exists: boolean;
  loaded: boolean;
  hasDirtyDocument: boolean;
  hasStaleDocument: boolean;
  hasConflictDocument: boolean;
}
