import type { GraphResourceKind } from "@/shared/types/domain/graphResourcePath";

export type ResourceKind = GraphResourceKind | "worksheet" | "database" | "variable";

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
  parentId?: string;
  scope?: { type: "global" | "event" | "function"; graphPath?: string };
  revision?: number;
  exists: boolean;
  loaded: boolean;
  hasDirtyDocument: boolean;
  hasStaleDocument: boolean;
  hasConflictDocument: boolean;
}

export interface BackendProjectResourceMeta {
  id: string;
  kind: ResourceKind;
  name: string;
  uri: string;
  exists: boolean;
  loaded: boolean;
  hasDirtyDocument: boolean;
  hasStaleDocument: boolean;
  hasConflictDocument: boolean;
}
