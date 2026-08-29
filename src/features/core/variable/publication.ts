import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { useVariableStore } from '@/features/core/dataStore/variableStore';
import type { VariableId, Variable } from '@/shared/types/domain';
import type { VariableReadSnapshot } from './read';

export interface VariablePublicationCapability {
  readonly replaceSnapshot: (snapshot: VariableReadSnapshot) => void;
  readonly publishVariable: (variable: DeepReadonly<Variable>, revision?: number) => void;
  readonly publishVariableRevision: (id: VariableId, revision: number) => void;
  readonly removeVariable: (id: VariableId) => void;
  readonly clearForProject: () => void;
}

function clone<T>(value: T): T {
  return structuredClone(value) as unknown as T;
}

export function createVariablePublication(): VariablePublicationCapability {
  return {
    replaceSnapshot: (snapshot) => useVariableStore.setState({
      variables: clone(snapshot.variables) as unknown as Record<VariableId, Variable>,
      revisions: clone(snapshot.revisions) as unknown as Record<VariableId, number>,
    }),

    publishVariable: (variable, revision) => useVariableStore.setState((state) => ({
      variables: {
        ...state.variables,
        [variable.id]: clone(variable) as unknown as Variable,
      },
      revisions: revision === undefined
        ? state.revisions
        : { ...state.revisions, [variable.id]: revision },
    })),

    publishVariableRevision: (id, revision) => useVariableStore.setState((state) => ({
      revisions: { ...state.revisions, [id]: revision },
    })),

    removeVariable: (id) => useVariableStore.setState((state) => {
      const variables = { ...state.variables };
      const revisions = { ...state.revisions };
      delete variables[id];
      delete revisions[id];
      return { variables, revisions };
    }),

    clearForProject: () => useVariableStore.setState({ variables: {}, revisions: {} }),
  };
}
