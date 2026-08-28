import type { SerializedDockview } from 'dockview-react';
import { logsDockviewLayoutController } from './logsDockviewLayoutController';

export interface LogsDockviewControl {
  beginRestore(): number;
  stageRestore(epoch: number, layout: SerializedDockview): 'staged' | 'applied' | 'stale';
  captureBoundSnapshot(): void;
  resetToDefault(): void;
}

export const logsDockviewControl: LogsDockviewControl = {
  beginRestore: logsDockviewLayoutController.beginRestore,
  stageRestore: logsDockviewLayoutController.stageRestore,
  captureBoundSnapshot: logsDockviewLayoutController.captureBoundSnapshot,
  resetToDefault: logsDockviewLayoutController.resetToDefault,
};
