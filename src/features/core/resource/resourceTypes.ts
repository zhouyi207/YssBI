import type { LayoutTab } from '@/shared/types/ui';

export type ResourceKind = 'event' | 'function' | 'worksheet' | 'database' | 'variable';

export type ResourceKey =
  | `graph:event:${string}`
  | `graph:function:${string}`
  | `worksheet:${string}`
  | `database:${string}`
  | `variable:${string}`;

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
  scope?: { type: 'global' | 'event' | 'function'; graphId?: string };
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

export function resourceKey(ref: ResourceRef): ResourceKey {
  switch (ref.kind) {
    case 'event':
    case 'function':
      return `graph:${ref.kind}:${ref.id}`;
    case 'worksheet':
      return `worksheet:${ref.id}`;
    case 'database':
      return `database:${ref.id}`;
    case 'variable':
      return `variable:${ref.id}`;
  }
}

export function graphResourceRef(id: string, kind: 'event' | 'function'): ResourceRef {
  return { id, kind };
}

export function resourceRefFromLayoutTab(tab: LayoutTab): ResourceRef | null {
  if (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet') {
    return { id: tab.id, kind: tab.type };
  }
  return null;
}

export function normalizeBackendResourceMeta(meta: BackendProjectResourceMeta): ProjectResourceMeta {
  return { ...meta };
}
