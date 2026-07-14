import { useTabBarReorderStore } from '@/features/application/editor/tabBarReorderStore';
import { editorDragChipClass } from '@/views/EditorView/Layout/editorDropPreviewStyles';

/** Floating editor-group ghost while dragging the tab strip background. */
export function EditorGroupDragOverlay() {
  const activeGroupDrag = useTabBarReorderStore((state) => state.activeGroupDrag);
  if (!activeGroupDrag) return null;

  const suffix = activeGroupDrag.tabCount > 1 ? ` (+${activeGroupDrag.tabCount - 1})` : '';

  return (
    <div className={`${editorDragChipClass} shadow-lg ring-1 ring-primary/40`}>
      <span className="max-w-[200px] truncate">{activeGroupDrag.title}{suffix}</span>
    </div>
  );
}
