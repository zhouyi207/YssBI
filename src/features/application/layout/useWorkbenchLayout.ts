import { useEffect } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import {
  hydrateWorkbenchChrome,
  hydrateWorkbenchLayout,
  reclampWorkbenchPanelSize,
  subscribeWorkbenchViewportResize,
} from '@/features/core/layout/workbenchLayoutService';
import { setWorkbenchLayoutWindowScope } from '@/features/core/layout/workbenchLayoutMemento';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';
import { reconcileOpenLayoutTabsWithResources } from '@/features/application/editor/reconcileOpenLayoutTabs';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { persistEditorTabsDebounced } from '@/features/core/layout/workbenchLayoutService';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { LoadStatus } from '@/shared/types/ui/common';

/** Hydrate persisted workbench chrome + editor grid from localStorage on mount. */
export function useWorkbenchLayout(): void {
  useEffect(() => {
    setWorkbenchLayoutWindowScope(getCurrentWindow().label);
    if (useProjectIOStore.getState().status === LoadStatus.Ready) {
      hydrateWorkbenchChrome();
    } else {
      hydrateWorkbenchLayout();
    }
    reconcileOpenLayoutTabsWithResources();
    reclampWorkbenchPanelSize();
    const activeEditorGroupId = useLayoutStore.getState().activeEditorGroupId;
    if (activeEditorGroupId) {
      void bootstrapEditorGraphSession(activeEditorGroupId);
    }
    const unsubscribeTabs = useEditorTabStore.subscribe(() => {
      persistEditorTabsDebounced();
    });
    const unsubscribeViewport = subscribeWorkbenchViewportResize();
    return () => {
      unsubscribeTabs();
      unsubscribeViewport();
    };
  }, []);
}
