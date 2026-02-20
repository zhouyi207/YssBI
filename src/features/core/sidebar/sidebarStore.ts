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

const DEFAULT_SECTIONS: Record<string, boolean> = {
  graphsEvent: true,
  graphsFunction: true,
  graphsMacro: true,
  graphsVariable: true,
  variablesGlobal: true,
  variablesLocal: true,
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
      const next = { ...state.expandedSections, [key]: !(state.expandedSections[key] ?? true) };
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
