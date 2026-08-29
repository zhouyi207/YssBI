import type { DeepReadonly } from '@/shared/types/deepReadonly';
import { EMPTY_HISTORY_STATE, useHistoryStore, type HistoryStoreState } from './historyStore';
import type { HistoryProjectionSnapshot } from './read';

export interface HistoryProjectionPublication {
  replaceSnapshot(snapshot: DeepReadonly<HistoryProjectionSnapshot>): void;
  clearForProject(projectInstanceId: string | null): void;
}

export function createHistoryProjectionPublication(): HistoryProjectionPublication {
  return {
    replaceSnapshot: (snapshot) => {
      useHistoryStore.setState({
        canUndo: snapshot.canUndo,
        canRedo: snapshot.canRedo,
        pending: snapshot.pending,
      });
    },
    clearForProject: (_projectInstanceId) => {
      useHistoryStore.setState({ ...EMPTY_HISTORY_STATE } satisfies HistoryStoreState);
    },
  };
}
