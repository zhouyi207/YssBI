import { useActivityEditorDragOverlayLabel } from "@/features/application/editor/editorDragDropActions";
import { SidebarDragOverlay } from "@/modules/workbench/public";

export function ActivityEditorDndOverlay() {
  return <SidebarDragOverlay label={useActivityEditorDragOverlayLabel()} />;
}
