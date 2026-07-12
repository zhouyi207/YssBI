import { useTabBarReorderStore } from '@/features/application/editor/tabBarReorderStore';
import { editorDragChipClass } from '@/views/EditorView/Layout/editorDropPreviewStyles';

/** Floating tab ghost while dragging — source tab stays as an invisible placeholder in the strip. */
export function TabDragOverlay() {
  const activeTabDrag = useTabBarReorderStore((state) => state.activeTabDrag);
  if (!activeTabDrag) return null;

  return (
    <div className={`${editorDragChipClass} shadow-lg ring-1 ring-primary/40`}>
      <span className="max-w-[160px] truncate">{activeTabDrag.title}</span>
    </div>
  );
}
