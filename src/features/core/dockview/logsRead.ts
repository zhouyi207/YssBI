import type { DeepReadonly } from '@/features/core/projection/deepReadonly';
import type { SerializedDockview } from 'dockview-react';
import { logsDockviewLayoutController } from './logsDockviewLayoutController';

export interface LogsDockviewRead {
  subscribe(listener: () => void): () => void;
  getLatestSnapshot(): DeepReadonly<SerializedDockview>;
}

export const logsDockviewRead: LogsDockviewRead = {
  subscribe: logsDockviewLayoutController.subscribe,
  getLatestSnapshot: logsDockviewLayoutController.getLatestSnapshot,
};
