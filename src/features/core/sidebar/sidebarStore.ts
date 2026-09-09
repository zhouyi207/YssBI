/** Project sidebar category expansion backed by localStorage. */
import { create } from "zustand";
import {
  PROJECT_TREE_CATEGORY_IDS,
  PROJECT_TREE_EXPANSION_DEFAULTS,
  type ProjectTreeCategoryId,
} from "./projectTreeState";

export {
  PROJECT_TREE_CATEGORY_IDS,
  PROJECT_TREE_EXPANSION_DEFAULTS,
  type ProjectTreeCategoryId,
} from "./projectTreeState";

const PROJECT_TREE_EXPANDED_CATEGORIES_KEY = "yssbi-project-tree-expanded-categories";

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

function isProjectTreeCategoryId(value: string): value is ProjectTreeCategoryId {
  return Object.values(PROJECT_TREE_CATEGORY_IDS).includes(value as ProjectTreeCategoryId);
}

function loadProjectTreeExpandedCategories(): Record<ProjectTreeCategoryId, boolean> {
  const persisted = loadFromStorage<Record<string, unknown>>(
    PROJECT_TREE_EXPANDED_CATEGORIES_KEY,
    {},
  );
  const filtered = Object.fromEntries(
    Object.entries(persisted).filter(
      ([categoryId, expanded]) =>
        isProjectTreeCategoryId(categoryId) && typeof expanded === "boolean",
    ),
  ) as Partial<Record<ProjectTreeCategoryId, boolean>>;
  return { ...PROJECT_TREE_EXPANSION_DEFAULTS, ...filtered };
}

export interface SidebarStore {
  projectTreeExpandedCategories: Record<ProjectTreeCategoryId, boolean>;
  setProjectTreeCategoryExpanded(categoryId: ProjectTreeCategoryId, expanded: boolean): void;
}

export const useSidebarStore = create<SidebarStore>((set) => ({
  projectTreeExpandedCategories: loadProjectTreeExpandedCategories(),

  setProjectTreeCategoryExpanded: (categoryId, expanded) =>
    set((state) => {
      const projectTreeExpandedCategories = {
        ...state.projectTreeExpandedCategories,
        [categoryId]: expanded,
      };
      saveToStorage(PROJECT_TREE_EXPANDED_CATEGORIES_KEY, projectTreeExpandedCategories);
      return { projectTreeExpandedCategories };
    }),
}));
