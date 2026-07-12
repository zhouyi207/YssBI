import { setWorkbenchPartVisible } from '@/features/core/layout/workbenchLayoutService';
import { useLayoutStore } from '@/features/core/layout/layoutStore';

export function ensureDetailVisible(): void {
  const layoutStore = useLayoutStore.getState();
  const detailNode = layoutStore.nodes.detail;
  if (!detailNode) return;

  if (detailNode.data?.userHidden === true) return;
  if (detailNode.data?.visible !== false) return;

  setWorkbenchPartVisible('detail', true);
}
