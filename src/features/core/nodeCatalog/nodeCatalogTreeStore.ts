import { create } from "zustand";

export interface NodeCatalogTreeState {
  scopeKey: string | null;
  expandedCategoryIds: Set<string>;
  setScope: (scopeKey: string | null) => void;
  setCategoryExpanded: (categoryId: string, expanded: boolean) => void;
  reset: () => void;
}

function initialState(scopeKey: string | null = null) {
  return {
    scopeKey,
    expandedCategoryIds: new Set<string>(),
  };
}

export const useNodeCatalogTreeStore = create<NodeCatalogTreeState>((set) => ({
  ...initialState(),

  setScope: (scopeKey) =>
    set((state) => (state.scopeKey === scopeKey ? state : initialState(scopeKey))),

  setCategoryExpanded: (categoryId, expanded) =>
    set((state) => {
      const expandedCategoryIds = new Set(state.expandedCategoryIds);
      if (expanded) expandedCategoryIds.add(categoryId);
      else expandedCategoryIds.delete(categoryId);
      return { expandedCategoryIds };
    }),

  reset: () => set(initialState()),
}));
