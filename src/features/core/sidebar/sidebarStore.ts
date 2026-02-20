/**
 * Sidebar UI 状态 Store
 * 管理侧边栏折叠/展开状态，支持 localStorage 持久化
 */
import { create } from "zustand";

const SECTIONS_KEY = "yssbi-sidebar-sections";
const DATAFRAMES_KEY = "yssbi-sidebar-dataframes";

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

/** 堆叠列表：同一 group 内仅一个 section 可展开 */
const SECTION_GROUPS: Record<string, string[]> = {
  graphs: ["graphsEvent", "graphsFunction", "graphsMacro", "graphsVariable"],
  variables: ["variablesGlobal", "variablesLocal"],
  data: ["dataData"],
};

const DEFAULT_SECTIONS: Record<string, boolean> = {
  graphsEvent: true,
  graphsFunction: false,
  graphsMacro: false,
  graphsVariable: false,
  variablesGlobal: true,
  variablesLocal: false,
  dataData: true,
};

export interface SidebarStore {
  expandedSections: Record<string, boolean>;
  expandedDataFrames: Record<string, boolean>;
  toggleSection: (key: string) => void;
  toggleDataFrame: (id: string) => void;
  isSectionExpanded: (key: string, defaultExpanded?: boolean) => boolean;
  isDataFrameExpanded: (id: string) => boolean;
}

export const useSidebarStore = create<SidebarStore>((set, get) => ({
  expandedSections: { ...DEFAULT_SECTIONS, ...loadFromStorage(SECTIONS_KEY, {}) },
  expandedDataFrames: loadFromStorage(DATAFRAMES_KEY, {}),

  toggleSection: (key: string) => {
    set((state) => {
      const current = state.expandedSections[key] ?? false;
      const next = { ...state.expandedSections };

      const group = Object.values(SECTION_GROUPS).find((g) => g.includes(key));
      if (group) {
        if (current) {
          next[key] = false;
        } else {
          group.forEach((k) => { next[k] = k === key; });
        }
      } else {
        next[key] = !current;
      }
      saveToStorage(SECTIONS_KEY, next);
      return { expandedSections: next };
    });
  },

  toggleDataFrame: (id: string) => {
    set((state) => {
      const next = { ...state.expandedDataFrames, [id]: !state.expandedDataFrames[id] };
      saveToStorage(DATAFRAMES_KEY, next);
      return { expandedDataFrames: next };
    });
  },

  isSectionExpanded: (key: string, defaultExpanded = true) => {
    return get().expandedSections[key] ?? defaultExpanded;
  },

  isDataFrameExpanded: (id: string) => get().expandedDataFrames[id] ?? false,
}));
