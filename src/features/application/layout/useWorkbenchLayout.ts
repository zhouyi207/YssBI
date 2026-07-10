import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  hydrateWorkbenchLayout,
  reclampWorkbenchPanelSize,
  subscribeWorkbenchViewportResize,
} from '@/features/core/layout/workbenchLayoutService';
import { setWorkbenchLayoutWindowScope } from '@/features/core/layout/workbenchLayoutMemento';

/** Hydrate persisted workbench chrome + editor grid from localStorage on mount. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    setWorkbenchLayoutWindowScope(getCurrentWindow().label);
    hydrateWorkbenchLayout();
    reclampWorkbenchPanelSize();
    return subscribeWorkbenchViewportResize();
  }, []);
}
