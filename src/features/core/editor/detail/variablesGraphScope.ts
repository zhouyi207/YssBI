import {
  getActiveLayoutTab,
  resolveEditorGroupId,
} from '@/features/core/layout/layoutTabQueries';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from '../stores/useEditorStore';

function readActiveGraphTab(): { id: string; type: 'event' | 'function' } | null {
  const layout = useLayoutStore.getState();
  const groupId = resolveEditorGroupId(undefined, layout);
  if (!groupId) return null;

  const active = getActiveLayoutTab(groupId, layout.nodes);
  if (active?.tab.type === 'event' || active?.tab.type === 'function') {
    return { id: active.activeTabId, type: active.tab.type };
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
