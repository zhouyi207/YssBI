import { SidebarDragOverlay } from './SidebarDragOverlay';
import { TabDragOverlay } from './TabDragOverlay';
import { useTabBarReorderStore } from '@/features/application/editor/tabBarReorderStore';

/** DragOverlay content — tab ghost takes priority over sidebar spawn overlay. */
export function WorkspaceDragOverlay() {
  const activeTabDrag = useTabBarReorderStore((state) => state.activeTabDrag);
  if (activeTabDrag) return <TabDragOverlay />;
  return <SidebarDragOverlay />;
}
