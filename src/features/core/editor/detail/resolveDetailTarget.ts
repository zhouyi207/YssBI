import type { DetailTarget, DetailTargetInput } from './types';

const TAB_DETAIL_TYPES = new Set(['event', 'function', 'worksheet']);

export function resolveDetailTarget(input: DetailTargetInput): DetailTarget | null {
  const { activeTabId, tabs, selectedNodeIds, sidebarDetailFocus, selectedLog } = input;

  if (selectedNodeIds.length === 1 && activeTabId) {
    return { kind: 'node', id: selectedNodeIds[0], graphId: activeTabId };
  }

  if (sidebarDetailFocus) {
    return { kind: sidebarDetailFocus.type, id: sidebarDetailFocus.id };
  }

  if (selectedLog) {
    return { kind: 'log' };
  }

  if (activeTabId) {
    const activeTab = tabs.find((tab) => tab.id === activeTabId);
    if (activeTab?.type && TAB_DETAIL_TYPES.has(activeTab.type)) {
      return { kind: activeTab.type as 'event' | 'function' | 'worksheet', id: activeTabId };
    }
  }

  return null;
}
