import { editorDockviewPort } from '@/features/core/dockview';
import type { LayoutTab } from '@/shared/types';
import { useEditorStore } from '../stores/useEditorStore';

function readActiveGraphTab(): { id: string; type: 'event' | 'function' } | null {
  const value = editorDockviewPort.getActivePanel()?.tab?.data?.layoutTab;
  const tab = value && typeof value === 'object' ? value as LayoutTab : null;
  if (tab?.type === 'event' || tab?.type === 'function') {
    return { id: tab.id, type: tab.type };
  }
  return null;
}

export function syncVariablesGraphScopeFromActiveTab(): void {
  const activeGraph = readActiveGraphTab();
  if (activeGraph) {
    useEditorStore.getState().setVariablesGraphScope(activeGraph.id);
  }
}

export function syncVariablesGraphScopeAfterClose(closedGraphPath: string): void {
  const store = useEditorStore.getState();
  const activeGraph = readActiveGraphTab();

  if (activeGraph) {
    store.setVariablesGraphScope(activeGraph.id);
    return;
  }

  if (store.variablesGraphScopePath === closedGraphPath) {
    return;
  }
}

export function setVariablesGraphScopeFromResource(graphPath: string): void {
  useEditorStore.getState().setVariablesGraphScope(graphPath);
}
