import { useSyncExternalStore } from 'react';

import type { DockviewEditorPort } from './dockviewEditorPort';
import type { DockviewPortSnapshot } from './types';

export function useDockviewPortSnapshot(
  port: DockviewEditorPort,
): DockviewPortSnapshot {
  return useSyncExternalStore(
    port.subscribe,
    port.getSnapshot,
    port.getSnapshot,
  );
}
