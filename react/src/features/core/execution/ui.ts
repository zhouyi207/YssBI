import { useExecutionStore } from "./useExecutionStore";

export interface RunFailureActions {
  readonly clearRunFailure: (graphPath: string) => void;
}

export const runFailureActions: RunFailureActions = {
  clearRunFailure: (graphPath) => useExecutionStore.getState().clearRunFailure(graphPath),
};
