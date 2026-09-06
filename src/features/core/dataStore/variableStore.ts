import { create } from "zustand";
import type { VariableId, Variable } from "@/shared/types";

interface VariableStore {
  variables: Record<VariableId, Variable>;
  revisions: Record<VariableId, number>;

  clearGraphVariables(graphPath: string): void;

  setVariableSnapshot(
    variables: Record<VariableId, Variable>,
    revisions: Record<VariableId, number>,
  ): void;
  clear(): void;
}

export const useVariableStore = create<VariableStore>((set) => ({
  variables: {},
  revisions: {},

  clearGraphVariables: (graphPath) =>
    set((state) => {
      const variables = { ...state.variables };
      const revisions = { ...state.revisions };
      for (const [id, variable] of Object.entries(state.variables)) {
        const scope = variable.scope;
        if (
          (scope.type === "event" && scope.eventPath === graphPath) ||
          (scope.type === "function" && scope.functionPath === graphPath)
        ) {
          delete variables[id];
          delete revisions[id];
        }
      }
      return { variables, revisions };
    }),

  setVariableSnapshot: (variables, revisions) => set({ variables, revisions }),
  clear: () => set({ variables: {}, revisions: {} }),
}));
