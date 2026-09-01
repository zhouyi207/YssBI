import { editorDragChipClass } from "./editorDropPreviewStyles";

/** Floating chip while dragging from sidebar (node template or graph resource). */
export function SidebarDragOverlay({ label }: { readonly label: string | null }) {
  if (!label) return null;

  return (
    <div className={`${editorDragChipClass} shadow-lg ring-1 ring-primary/40`}>
      <span className="h-2 w-2 shrink-0 rounded-full bg-primary" />
      <span className="max-w-[160px] truncate">{label}</span>
    </div>
  );
}
