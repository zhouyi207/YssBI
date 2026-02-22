import { create } from 'zustand';
import type { VariableId, Variable } from '@/shared/types';
import { logger } from '@/utils/appLogger';

interface VariableStore {
  variables: Record<VariableId, Variable>;

  addVariable(id: VariableId, v: Variable): void;
  updateVariable(id: VariableId, patch: Partial<Variable>): void;
  deleteVariable(id: VariableId): void;

  setVariables(vars: Record<VariableId, Variable>): void;
  clear(): void;
}

export const useVariableStore = create<VariableStore>((set) => ({
  variables: {},

  // ==========================
  // CRUD
  // ==========================
  addVariable: (id, v) =>
    set((state) => {
      if (state.variables[id]) {
        logger.data.warn(`addVariable: Variable "${id}" already exists`, 'VariableStore');
        return state;
      }
      return { variables: { ...state.variables, [id]: v } };
    }),

  updateVariable: (id, patch) =>
    set((state) => {
      const prev = state.variables[id];
      if (!prev) {
        logger.data.warn(`updateVariable: Variable "${id}" not found`, 'VariableStore');
        return state;
      }
      return {
        variables: { ...state.variables, [id]: { ...prev, ...patch } },
      };
    }),

  deleteVariable: (id) =>
    set((state) => {
      if (!state.variables[id]) {
        logger.data.warn(`deleteVariable: Variable "${id}" not found`, 'VariableStore');
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
