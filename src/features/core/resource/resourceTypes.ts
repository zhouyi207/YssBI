import type { LayoutTab } from '@/shared/types/ui';
import {
  toGraphResourceUri,
  type GraphResourceKind,
} from '@/shared/types/domain/graphResourcePath';

export type ResourceKind = 'event' | 'function' | 'worksheet' | 'database' | 'variable';

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
  scope?: { type: 'global' | 'event' | 'function'; graphPath?: string };
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

type ResourceKeyInput = ResourceRef | Pick<ProjectResourceMeta, 'kind' | 'id' | 'uri'>;

export function resourceKey(input: ResourceKeyInput): ResourceKey {
  if ('uri' in input && input.uri) {
    return input.uri;
  }
  return resourceKeyFromRef(input as ResourceRef);
}

function resourceKeyFromRef(ref: ResourceRef): ResourceKey {
  switch (ref.kind) {
    case 'event':
    case 'function':
      return toGraphResourceUri(ref.kind, ref.id);
    case 'worksheet':
      return `yssbi://worksheet/${ref.id}`;
    case 'database':
      return `yssbi://database/${ref.id}`;
    case 'variable':
      return `yssbi://variable/${ref.id}`;
  }
}

export function buildGraphResourceMeta(
  kind: GraphResourceKind,
  path: string,
  name: string,
  overrides?: Partial<Omit<ProjectResourceMeta, 'id' | 'kind' | 'name' | 'uri'>>,
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

export function resourceRefFromLayoutTab(tab: LayoutTab): ResourceRef | null {
  if (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet') {
    return { id: tab.id, kind: tab.type };
  }
  return null;
}
