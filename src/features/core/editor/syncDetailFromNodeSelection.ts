import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { useEditorStore } from './stores/useEditorStore';

export function syncDetailFromNodeSelection(
  groupId: string,
  selectedNodeIds: string[],
): void {
  const layoutNode = useLayoutStore.getState().nodes[groupId];
  const tabId = layoutNode?.data?.activeTabId ?? null;
  const { selectedItemType, setSelectedInfo } = useEditorStore.getState();

  if (selectedNodeIds.length === 1 && tabId) {
    setSelectedInfo(selectedNodeIds[0], 'node', tabId);
    return;
  }

  if (selectedItemType === 'node') {
    setSelectedInfo(null, null);
  }
}
