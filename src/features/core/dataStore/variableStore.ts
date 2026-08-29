import { create } from 'zustand';
import type { VariableId, Variable } from '@/shared/types';
import { logger } from '@/features/core/observability/logger';

interface VariableStore {
  variables: Record<VariableId, Variable>;
  revisions: Record<VariableId, number>;

  addVariable(id: VariableId, v: Variable): void;
  updateVariable(id: VariableId, patch: Partial<Variable>): void;
  deleteVariable(id: VariableId): void;
  clearGraphVariables(graphPath: string): void;

  setVariables(vars: Record<VariableId, Variable>): void;
  setVariableSnapshot(
    vars: Record<VariableId, Variable>,
    revisions: Record<VariableId, number>,
  ): void;
  setVariableRevision(id: VariableId, revision: number): void;
  clear(): void;
}

export const useVariableStore = create<VariableStore>((set) => ({
  variables: {},
  revisions: {},

  // ==========================
  // CRUD
  // ==========================
  addVariable: (id, v) =>
    set((state) => {
      if (state.variables[id]) {
        logger.data.warn(`addVariable: Variable "${id}" already exists`, 'VariableStore');
        return state;
      }
      return {
        variables: { ...state.variables, [id]: v },
        revisions: { ...state.revisions, [id]: state.revisions[id] ?? 0 },
      };
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
      const nextRevisions = { ...state.revisions };
      delete nextVars[id];
      delete nextRevisions[id];
      return { variables: nextVars, revisions: nextRevisions };
    }),

  clearGraphVariables: (graphPath) =>
    set((state) => {
      const nextVars = { ...state.variables };
      const nextRevisions = { ...state.revisions };
      for (const [id, variable] of Object.entries(state.variables)) {
        const scope = variable.scope;
        if (
          (scope.type === 'event' && scope.eventPath === graphPath) ||
          (scope.type === 'function' && scope.functionPath === graphPath)
        ) {
          delete nextVars[id];
          delete nextRevisions[id];
        }
      }
      return { variables: nextVars, revisions: nextRevisions };
    }),

  // ==========================
  // Project / 全量设置
  // ==========================
  setVariables: (vars) => set({
    variables: vars,
    revisions: Object.fromEntries(Object.keys(vars).map((id) => [id, 0])),
  }),
  setVariableSnapshot: (variables, revisions) => set({ variables, revisions }),
  setVariableRevision: (id, revision) => set((state) => ({
    revisions: { ...state.revisions, [id]: revision },
  })),
  clear: () => set({ variables: {}, revisions: {} }),
}));
