import type { LayoutNode, LayoutTab } from '@/shared/types';
import { useEditorTabStore } from './editorTabStore';

type LayoutNodeData = NonNullable<LayoutNode['data']>;

export function resetEditorTabStore(): void {
  useEditorTabStore.setState({ registry: {}, placements: {} });
}

export function seedEditorGroupTabs(
  groupId: string,
  tabs: LayoutTab[],
  activeTabId?: string | null,
  selectedNodeIds: string[] = [],
): void {
  useEditorTabStore.getState().initGroupPlacement(
    groupId,
    tabs,
    activeTabId ?? (tabs.length > 0 ? tabs[tabs.length - 1].id : null),
  );
  if (selectedNodeIds.length > 0) {
    useEditorTabStore.getState().setSelectedNodeIds(groupId, selectedNodeIds);
  }
}

/** Embedded tabs on layout nodes — migration/import tests only. */
export function legacyEmbeddedNodeData(
  base: LayoutNodeData,
  tabs: LayoutTab[],
  activeTabId?: string | null,
  selectedNodeIds?: string[],
): LayoutNodeData {
  return {
    ...base,
    tabs,
    activeTabId,
    params: selectedNodeIds ? { selectedNodeIds } : base.params,
  } as LayoutNodeData;
}
