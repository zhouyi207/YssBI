import { useSyncExternalStore } from 'react';

import type { DeepReadonly } from '@/shared/types/deepReadonly';
import {
  useSidebarStore,
  type ProjectTreeCategoryId,
  type SidebarSectionKey,
} from './sidebarStore';

export interface SidebarUiSnapshot {
  readonly expandedSections: DeepReadonly<Record<string, boolean>>;
  readonly projectTreeQuery: string;
  readonly projectTreeExpandedCategories: DeepReadonly<Record<ProjectTreeCategoryId, boolean>>;
}

export interface SidebarUiCapability {
  readonly getSnapshot: () => DeepReadonly<SidebarUiSnapshot>;
  readonly subscribe: (listener: () => void) => () => void;
  readonly toggleSection: (key: SidebarSectionKey) => void;
  readonly setSectionExpanded: (key: SidebarSectionKey, expanded: boolean) => void;
  readonly setProjectTreeQuery: (query: string) => void;
  readonly setProjectTreeCategoryExpanded: (
    categoryId: ProjectTreeCategoryId,
    expanded: boolean,
  ) => void;
  readonly setProjectTreeCategoriesExpanded: (
    categoryIds: Iterable<ProjectTreeCategoryId>,
    expanded: boolean,
  ) => void;
  readonly resetProjectTreeQuery: () => void;
}

function cloneAndFreeze<T>(value: T): T {
  if (value === null || typeof value !== 'object') return value;
  return Object.freeze(Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .map(([key, nested]) => [key, cloneAndFreeze(nested)]),
  )) as T;
}

function buildSnapshot(): DeepReadonly<SidebarUiSnapshot> {
  const state = useSidebarStore.getState();
  return Object.freeze({
    expandedSections: cloneAndFreeze(state.expandedSections),
    projectTreeQuery: state.projectTreeQuery,
    projectTreeExpandedCategories: cloneAndFreeze(state.projectTreeExpandedCategories),
  });
}

let currentSnapshot = buildSnapshot();
const listeners = new Set<() => void>();

function refreshSnapshot(): void {
  currentSnapshot = buildSnapshot();
  for (const listener of listeners) listener();
}

useSidebarStore.subscribe(refreshSnapshot);

export function getSidebarUiSnapshot(): DeepReadonly<SidebarUiSnapshot> {
  return currentSnapshot;
}

export function subscribeSidebarUi(listener: () => void): () => void {
  listeners.add(listener);
  return () => listeners.delete(listener);
}

export function useSidebarUi<T>(
  selector: (snapshot: DeepReadonly<SidebarUiSnapshot>) => T,
): T {
  const snapshot = useSyncExternalStore(
    subscribeSidebarUi,
    getSidebarUiSnapshot,
    getSidebarUiSnapshot,
  );
  return selector(snapshot);
}

export const sidebarUi: SidebarUiCapability = {
  getSnapshot: getSidebarUiSnapshot,
  subscribe: subscribeSidebarUi,
  toggleSection: (key) => useSidebarStore.getState().toggleSection(key),
  setSectionExpanded: (key, expanded) =>
    useSidebarStore.getState().setSectionExpanded(key, expanded),
  setProjectTreeQuery: (query) => useSidebarStore.getState().setProjectTreeQuery(query),
  setProjectTreeCategoryExpanded: (categoryId, expanded) =>
    useSidebarStore.getState().setProjectTreeCategoryExpanded(categoryId, expanded),
  setProjectTreeCategoriesExpanded: (categoryIds, expanded) =>
    useSidebarStore.getState().setProjectTreeCategoriesExpanded(categoryIds, expanded),
  resetProjectTreeQuery: () => useSidebarStore.getState().resetProjectTreeQuery(),
};
