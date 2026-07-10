import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  hydrateWorkbenchLayout,
  reclampWorkbenchPanelSize,
  subscribeWorkbenchViewportResize,
} from '@/features/core/layout/workbenchLayoutService';
import { setWorkbenchLayoutWindowScope } from '@/features/core/layout/workbenchLayoutMemento';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { activateCurrentEditorTab } from '@/features/application/editor/switchEditorTab';

/** Hydrate persisted workbench chrome + editor grid from localStorage on mount. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    setWorkbenchLayoutWindowScope(getCurrentWindow().label);
    hydrateWorkbenchLayout();
    reclampWorkbenchPanelSize();
    const activeGroupId = useLayoutStore.getState().activeEditorGroupId;
    if (activeGroupId) {
      void activateCurrentEditorTab(activeGroupId);
    }
    return subscribeWorkbenchViewportResize();
  }, []);
}
