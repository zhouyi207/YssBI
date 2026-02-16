import { create } from 'zustand';
import { VariableId, VariableData } from '@/shared/types';

interface VariableStore {
  variables: Record<VariableId, VariableData>;

  addVariable(id: VariableId, v: VariableData): void;
  updateVariable(id: VariableId, patch: Partial<VariableData>): void;
  deleteVariable(id: VariableId): void;

  setVariables(vars: Record<VariableId, VariableData>): void;
  clear(): void;
}

export const useVariableStore = create<VariableStore>((set, get) => ({
  variables: {},

  // ==========================
  // CRUD
  // ==========================
  addVariable: (id, v) =>
    set((state) => {
      if (state.variables[id]) {
        console.warn(`[VariableStore] addVariable: Variable "${id}" already exists`);
        return state;
      }
      return { variables: { ...state.variables, [id]: v } };
    }),

  updateVariable: (id, patch) =>
    set((state) => {
      const prev = state.variables[id];
      if (!prev) {
        console.warn(`[VariableStore] updateVariable: Variable "${id}" not found`);
        return state;
      }
      return {
        variables: { ...state.variables, [id]: { ...prev, ...patch } },
      };
    }),

  deleteVariable: (id) =>
    set((state) => {
      if (!state.variables[id]) {
        console.warn(`[VariableStore] deleteVariable: Variable "${id}" not found`);
        return state;
      }
      const nextVars = { ...state.variables };
      delete nextVars[id];
      return { variables: nextVars };
    }),

  // ==========================
  // Project / 全量设置
  // ==========================
  setVariables: (vars) => set({ variables: vars }),
  clear: () => set({ variables: {} }),
}));
