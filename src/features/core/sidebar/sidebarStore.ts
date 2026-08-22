/** Sidebar UI state with localStorage-backed section and Project tree expansion. */
import { create } from 'zustand';
import {
  isSupportedSidebarSectionKey,
  mergeExpandedSections,
  resolveSectionExpanded,
  type SidebarSectionKey,
} from './sidebarSectionState';
import {
  PROJECT_TREE_CATEGORY_IDS,
  PROJECT_TREE_EXPANSION_DEFAULTS,
  type ProjectTreeCategoryId,
} from './projectTreeState';

export {
  SIDEBAR_SECTION_DEFAULTS,
  type SidebarSectionKey,
} from './sidebarSectionState';
export {
  PROJECT_TREE_CATEGORY_IDS,
  PROJECT_TREE_EXPANSION_DEFAULTS,
  type ProjectTreeCategoryId,
} from './projectTreeState';

export * from './flatRows';

const SECTIONS_KEY = 'yssbi-sidebar-sections';
const PROJECT_TREE_EXPANDED_CATEGORIES_KEY = 'yssbi-project-tree-expanded-categories';

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
  return mergeExpandedSections(loadFromStorage<Record<string, unknown>>(SECTIONS_KEY, {}));
}

function isProjectTreeCategoryId(value: string): value is ProjectTreeCategoryId {
  return Object.values(PROJECT_TREE_CATEGORY_IDS).includes(value as ProjectTreeCategoryId);
}

function loadProjectTreeExpandedCategories(): Record<ProjectTreeCategoryId, boolean> {
  const persisted = loadFromStorage<Record<string, unknown>>(
    PROJECT_TREE_EXPANDED_CATEGORIES_KEY,
    {},
  );
  const filtered = Object.fromEntries(
    Object.entries(persisted).filter(([categoryId, expanded]) => (
      isProjectTreeCategoryId(categoryId) && typeof expanded === 'boolean'
    )),
  ) as Partial<Record<ProjectTreeCategoryId, boolean>>;
  return { ...PROJECT_TREE_EXPANSION_DEFAULTS, ...filtered };
}

export interface SidebarStore {
  expandedSections: Record<string, boolean>;
  projectTreeQuery: string;
  projectTreeExpandedCategories: Record<ProjectTreeCategoryId, boolean>;
  toggleSection: (key: SidebarSectionKey) => void;
  setSectionExpanded: (key: SidebarSectionKey, expanded: boolean) => void;
  isSectionExpanded: (key: SidebarSectionKey) => boolean;
  setProjectTreeQuery(query: string): void;
  setProjectTreeCategoryExpanded(categoryId: ProjectTreeCategoryId, expanded: boolean): void;
  setProjectTreeCategoriesExpanded(
    categoryIds: Iterable<ProjectTreeCategoryId>,
    expanded: boolean,
  ): void;
  resetProjectTreeQuery(): void;
}

export const useSidebarStore = create<SidebarStore>((set, get) => ({
  expandedSections: loadExpandedSections(),
  projectTreeQuery: '',
  projectTreeExpandedCategories: loadProjectTreeExpandedCategories(),

  toggleSection: (key) => {
    if (!isSupportedSidebarSectionKey(key)) return;
    set((state) => {
      const current = resolveSectionExpanded(state.expandedSections, key);
      const next = mergeExpandedSections({ [key]: !current });
      saveToStorage(SECTIONS_KEY, next);
      return { expandedSections: next };
    });
  },

  setSectionExpanded: (key, expanded) => {
    if (!isSupportedSidebarSectionKey(key)) return;
    set(() => {
      const next = mergeExpandedSections({ [key]: expanded });
      saveToStorage(SECTIONS_KEY, next);
      return { expandedSections: next };
    });
  },

  isSectionExpanded: (key) => resolveSectionExpanded(get().expandedSections, key),

  setProjectTreeQuery: (projectTreeQuery) => set({ projectTreeQuery }),

  setProjectTreeCategoryExpanded: (categoryId, expanded) => set((state) => {
    const projectTreeExpandedCategories = {
      ...state.projectTreeExpandedCategories,
      [categoryId]: expanded,
    };
    saveToStorage(PROJECT_TREE_EXPANDED_CATEGORIES_KEY, projectTreeExpandedCategories);
    return { projectTreeExpandedCategories };
  }),

  setProjectTreeCategoriesExpanded: (categoryIds, expanded) => set((state) => {
    const projectTreeExpandedCategories = { ...state.projectTreeExpandedCategories };
    for (const categoryId of categoryIds) projectTreeExpandedCategories[categoryId] = expanded;
    saveToStorage(PROJECT_TREE_EXPANDED_CATEGORIES_KEY, projectTreeExpandedCategories);
    return { projectTreeExpandedCategories };
  }),

  resetProjectTreeQuery: () => set({ projectTreeQuery: '' }),
}));
