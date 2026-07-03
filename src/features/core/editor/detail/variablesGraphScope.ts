import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '../stores/useEditorStore';

function readActiveGraphTab(): { id: string; type: 'event' | 'function' } | null {
  const layout = useLayoutStore.getState();
  const editorNode = layout.activeEditorGroupId ? layout.nodes[layout.activeEditorGroupId] : null;
  const activeTabId = editorNode?.data?.activeTabId;
  if (!activeTabId) return null;

  const tab = editorNode?.data?.tabs?.find((item) => item.id === activeTabId);
  if (tab?.type === 'event' || tab?.type === 'function') {
    return { id: activeTabId, type: tab.type };
  }
  return null;
}

export function syncVariablesGraphScopeFromActiveTab(): void {
  const activeGraph = readActiveGraphTab();
  if (activeGraph) {
    useEditorStore.getState().setVariablesGraphScope(activeGraph.id);
  }
}

export function syncVariablesGraphScopeAfterClose(closedGraphId: string): void {
  const store = useEditorStore.getState();
  const activeGraph = readActiveGraphTab();

  if (activeGraph) {
    store.setVariablesGraphScope(activeGraph.id);
    return;
  }

  if (store.variablesGraphScopeId === closedGraphId) {
    return;
  }
}

export function setVariablesGraphScopeFromResource(graphId: string): void {
  useEditorStore.getState().setVariablesGraphScope(graphId);
}
