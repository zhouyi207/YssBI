import { useCallback, useEffect, useRef } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-react';

import { workbenchLayoutController } from './workbenchLayoutController';

/** Bind the sole root Dockview to the current window's layout lifecycle. */
export function useWorkbenchLayout(): (event: DockviewReadyEvent) => void {
  const boundApiRef = useRef<DockviewApi | null>(null);

  const bind = useCallback((event: DockviewReadyEvent) => {
    const previousApi = boundApiRef.current;
    if (previousApi && previousApi !== event.api) {
      workbenchLayoutController.unbind(previousApi);
    }

    workbenchLayoutController.bind(event.api, getCurrentWindow().label);
    boundApiRef.current = event.api;
  }, []);

  useEffect(() => () => {
    const boundApi = boundApiRef.current;
    boundApiRef.current = null;
    if (boundApi) workbenchLayoutController.unbind(boundApi);
  }, []);

  return bind;
}
