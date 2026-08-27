import { useCallback, useEffect, useMemo, useRef } from 'react';
import type { DockviewApi, DockviewReadyEvent } from 'dockview-react';
import { currentAppWindow } from '@/services/platform/appWindow';

import { workbenchLayoutController } from './workbenchLayoutController';

/** Bind the sole root Dockview to the current window's layout lifecycle. */
export function useWorkbenchLayout(): (event: DockviewReadyEvent) => void {
  const boundApiRef = useRef<DockviewApi | null>(null);
  const windowLabel = useMemo(() => currentAppWindow().label, []);

  const bind = useCallback((event: DockviewReadyEvent) => {
    const previousApi = boundApiRef.current;
    if (previousApi && previousApi !== event.api) {
      workbenchLayoutController.unbind(previousApi);
    }

    workbenchLayoutController.bind(event.api, windowLabel);
    boundApiRef.current = event.api;
  }, [windowLabel]);

  useEffect(() => () => {
    const boundApi = boundApiRef.current;
    boundApiRef.current = null;
    if (boundApi) workbenchLayoutController.unbind(boundApi);
  }, []);

  return bind;
}
