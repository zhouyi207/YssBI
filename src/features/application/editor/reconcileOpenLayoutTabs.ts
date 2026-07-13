import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import type { LayoutTabInput } from '@/features/core/layout/layoutTabModel';
import { normalizeLayoutTab } from '@/features/core/layout/layoutTabModel';

/**
 * Drop hydrate-only tab title snapshots after ResourceStore is authoritative.
 */
export function reconcileOpenLayoutTabsWithResources(): void {
  useEditorTabStore.setState((state) => {
    for (const [tabId, tab] of Object.entries(state.registry)) {
      const input = tab as LayoutTabInput;
      if (input.title === undefined) continue;
      const { title: _title, ...rest } = input;
      state.registry[tabId] = normalizeLayoutTab(rest);
    }
  });
}
