/**
 * Sidebar UI state — section/group expand with localStorage persistence.
 */
import { create } from 'zustand';
import { resolveGroupExpanded } from './flatRows/groupExpandState';
import {
  mergeExpandedSections,
  resolveSectionExpanded,
  type SidebarSectionKey,
} from './sidebarSectionState';

export {
  SIDEBAR_SECTION_DEFAULTS,
  type SidebarSectionKey,
} from './sidebarSectionState';

export * from './flatRows';

const SECTIONS_KEY = 'yssbi-sidebar-sections';
const GROUPS_KEY = 'yssbi-sidebar-groups';

function loadFromStorage<T>(key: string, fallback: T): T {
  try {
    const raw = localStorage.getItem(key);
    if (raw) {
      const parsed = JSON.parse(raw) as T;
      if (parsed != null) return parsed;
    }
  } catch {
    // ignore
  }
  return fallback;
}

function saveToStorage(key: string, value: unknown) {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch {
    // ignore
  }
}

function loadExpandedSections(): Record<string, boolean> {
  return mergeExpandedSections(loadFromStorage(SECTIONS_KEY, {}));
}

function loadExpandedGroups(): Record<string, boolean> {
  return loadFromStorage(GROUPS_KEY, {});
}

export interface SidebarStore {
  expandedSections: Record<string, boolean>;
  expandedGroups: Record<string, boolean>;
  toggleSection: (key: SidebarSectionKey) => void;
  setSectionExpanded: (key: SidebarSectionKey, expanded: boolean) => void;
  isSectionExpanded: (key: SidebarSectionKey) => boolean;
  toggleGroup: (groupKey: string) => void;
}

export const useSidebarStore = create<SidebarStore>((set, get) => ({
  expandedSections: loadExpandedSections(),
  expandedGroups: loadExpandedGroups(),

  toggleSection: (key) => {
    set((state) => {
      const current = resolveSectionExpanded(state.expandedSections, key);
      const next = { ...state.expandedSections, [key]: !current };
      saveToStorage(SECTIONS_KEY, next);
      return { expandedSections: next };
    });
  },

  setSectionExpanded: (key, expanded) => {
    set((state) => {
      const next = { ...state.expandedSections, [key]: expanded };
      saveToStorage(SECTIONS_KEY, next);
      return { expandedSections: next };
    });
  },

  isSectionExpanded: (key) => resolveSectionExpanded(get().expandedSections, key),

  toggleGroup: (groupKey) => {
    set((state) => {
      const current = resolveGroupExpanded(state.expandedGroups, groupKey);
      const next = { ...state.expandedGroups, [groupKey]: !current };
      saveToStorage(GROUPS_KEY, next);
      return { expandedGroups: next };
    });
  },
}));
