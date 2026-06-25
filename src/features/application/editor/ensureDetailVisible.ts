import { useLayoutStore } from '@/features/core/layout/layoutStore';

export function ensureDetailVisible(): void {
  const layoutStore = useLayoutStore.getState();
  const detailNode = layoutStore.nodes.detail;
  if (detailNode?.data?.visible === false) {
    layoutStore.updateNode('detail', {
      data: { ...detailNode.data, visible: true },
    });
  }
}
