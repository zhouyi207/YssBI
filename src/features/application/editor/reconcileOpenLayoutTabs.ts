import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { isEditorGroupNode } from '@/features/core/layout/layoutTabQueries';
import type { LayoutTabInput } from '@/features/core/layout/layoutTabModel';
import { normalizeLayoutTab } from '@/features/core/layout/layoutTabModel';

/**
 * Drop hydrate-only tab title snapshots after ResourceStore is authoritative.
 * Safe to call on every project load / resource index refresh.
 */
export function reconcileOpenLayoutTabsWithResources(): void {
  useLayoutStore.setState((state) => {
    const nodes = { ...state.nodes };
    let changed = false;

    for (const [nodeId, node] of Object.entries(state.nodes)) {
      if (!isEditorGroupNode(node) || !node.data?.tabs?.length) continue;
      const hasTitle = node.data.tabs.some(
        (tab) => (tab as LayoutTabInput).title !== undefined,
      );
      if (!hasTitle) continue;

      changed = true;
      nodes[nodeId] = {
        ...node,
        data: {
          ...node.data,
          tabs: node.data.tabs.map((tab) => {
            const { title: _title, ...rest } = tab as LayoutTabInput;
            return normalizeLayoutTab(rest);
          }),
        },
      };
    }

    return changed ? { nodes } : state;
  });
}
