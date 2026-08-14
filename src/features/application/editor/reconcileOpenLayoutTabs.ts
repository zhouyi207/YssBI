import { editorDockviewPort, useEditorPaneStateStore } from '@/features/core/dockview';
import { resourceKey, useResourceStore } from '@/features/core/resource';
import { layoutTabFromDockviewPanel } from './dockviewTabProjection';

/** Remove restored Dockview panels whose project resources no longer exist. */
export function reconcileOpenLayoutTabsWithResources(): void {
  const resources = useResourceStore.getState().resources;
  for (const panel of editorDockviewPort.listPanels()) {
    const tab = layoutTabFromDockviewPanel(panel);
    if (!tab || (tab.type !== 'event' && tab.type !== 'function' && tab.type !== 'worksheet')) continue;
    if (resources[resourceKey({ id: tab.id, kind: tab.type })]) continue;
    useEditorPaneStateStore.getState().release(panel.panelInstanceId);
    void editorDockviewPort.remove(panel.panelInstanceId);
  }
}
