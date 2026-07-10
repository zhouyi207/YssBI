import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  hydrateWorkbenchLayout,
  reclampWorkbenchPanelSize,
  subscribeWorkbenchViewportResize,
} from '@/features/core/layout/workbenchLayoutService';
import { setWorkbenchLayoutWindowScope } from '@/features/core/layout/workbenchLayoutMemento';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';

/** Hydrate persisted workbench chrome + editor grid from localStorage on mount. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    setWorkbenchLayoutWindowScope(getCurrentWindow().label);
    hydrateWorkbenchLayout();
    reclampWorkbenchPanelSize();
    const activeEditorGroupId = useLayoutStore.getState().activeEditorGroupId;
    if (activeEditorGroupId) {
      void bootstrapEditorGraphSession(activeEditorGroupId);
    }
    return subscribeWorkbenchViewportResize();
  }, []);
}
