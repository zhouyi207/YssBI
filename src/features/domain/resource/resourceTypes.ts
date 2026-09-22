import type { GraphResourceKind } from "@/shared/types/domain/graphResourcePath";

export type ResourceKind = GraphResourceKind | "chart" | "database";

/** Canonical store key — always equals `ProjectResourceMeta.uri`. */
export type ResourceKey = string;

export interface ResourceRef {
  kind: ResourceKind;
  id: string;
}

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
