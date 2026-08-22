import { create } from 'zustand';

export interface NodeCatalogTreeState {
  scopeKey: string | null;
  query: string;
  expandedCategoryIds: Set<string>;
  setScope: (scopeKey: string | null) => void;
  setQuery: (query: string) => void;
  setCategoryExpanded: (categoryId: string, expanded: boolean) => void;
  setCategoriesExpanded: (categoryIds: Iterable<string>, expanded: boolean) => void;
  reset: () => void;
}

function initialState(scopeKey: string | null = null) {
  return {
    scopeKey,
    query: '',
    expandedCategoryIds: new Set<string>(),
  };
}

export const useNodeCatalogTreeStore = create<NodeCatalogTreeState>((set) => ({
  ...initialState(),

  setScope: (scopeKey) => set((state) => (
    state.scopeKey === scopeKey ? state : initialState(scopeKey)
  )),

  setQuery: (query) => set({ query }),

  setCategoryExpanded: (categoryId, expanded) => set((state) => {
    const expandedCategoryIds = new Set(state.expandedCategoryIds);
    if (expanded) expandedCategoryIds.add(categoryId);
    else expandedCategoryIds.delete(categoryId);
    return { expandedCategoryIds };
  }),

  setCategoriesExpanded: (categoryIds, expanded) => set((state) => {
    const expandedCategoryIds = new Set(state.expandedCategoryIds);
    for (const categoryId of categoryIds) {
      if (expanded) expandedCategoryIds.add(categoryId);
      else expandedCategoryIds.delete(categoryId);
    }
    return { expandedCategoryIds };
  }),

  reset: () => set(initialState()),
}));
