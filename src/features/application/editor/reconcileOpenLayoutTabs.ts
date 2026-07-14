import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import type { LayoutTabInput } from '@/features/core/layout/layoutTabModel';
import { normalizeLayoutTab } from '@/features/core/layout/layoutTabModel';
import { resourceKey, useResourceStore } from '@/features/core/resource';

/**
 * Keep restored editor tabs aligned with the current project resource index.
 */
export function reconcileOpenLayoutTabsWithResources(): void {
  const resources = useResourceStore.getState().resources;
  useEditorTabStore.setState((state) => {
    for (const [tabId, tab] of Object.entries(state.registry)) {
      const input = tab as LayoutTabInput;
      if (tab.type === 'event' || tab.type === 'function' || tab.type === 'worksheet') {
        if (!resources[resourceKey({ id: tabId, kind: tab.type })]) {
          delete state.registry[tabId];
          continue;
        }
      }
      if (input.title !== undefined) {
        const { title: _title, ...rest } = input;
        state.registry[tabId] = normalizeLayoutTab(rest);
      }
    }

    for (const placement of Object.values(state.placements)) {
      placement.tabIds = placement.tabIds.filter((tabId) => Boolean(state.registry[tabId]));
      placement.selectedTabIds = placement.selectedTabIds.filter((tabId) => placement.tabIds.includes(tabId));
      if (!placement.activeTabId || !placement.tabIds.includes(placement.activeTabId)) {
        placement.activeTabId = placement.tabIds[placement.tabIds.length - 1] ?? null;
      }
    }
  });
}
