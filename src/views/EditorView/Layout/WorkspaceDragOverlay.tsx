import { SidebarDragOverlay } from './SidebarDragOverlay';
import { TabDragOverlay } from './TabDragOverlay';
import { EditorGroupDragOverlay } from './EditorGroupDragOverlay';
import { useTabBarReorderStore } from '@/features/application/editor/tabBarReorderStore';

/** DragOverlay content — tab ghost > group ghost > sidebar spawn overlay. */
export function WorkspaceDragOverlay() {
  const activeTabDrag = useTabBarReorderStore((state) => state.activeTabDrag);
  const activeGroupDrag = useTabBarReorderStore((state) => state.activeGroupDrag);
  if (activeTabDrag) return <TabDragOverlay />;
  if (activeGroupDrag) return <EditorGroupDragOverlay />;
  return <SidebarDragOverlay />;
}
