import { useLayoutStore } from '@/features/core/layout/layoutStore';
import type { ResourceRef } from './resourceTypes';

export function updateOpenResourceLabels(ref: ResourceRef, name: string): void {
  useLayoutStore.setState((state) => {
    for (const node of Object.values(state.nodes)) {
      const tab = node.data?.tabs?.find((item) => item.id === ref.id && item.type === ref.kind);
      if (tab) tab.title = name;
    }
  });
}
