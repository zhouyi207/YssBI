/** Shared shell for editor drop previews (split half / canvas open). */
export const editorDropPreviewShellClass =
  'pointer-events-none fixed z-[120] box-border rounded-sm border-2 border-primary bg-primary/15 shadow-[inset_0_0_0_1px_hsl(var(--primary)/0.35)] transition-[top,left,width,height,opacity] duration-100 ease-out';

export const editorDropPreviewLabelClass =
  'rounded-md border border-primary/40 bg-background/90 px-3 py-1.5 text-xs font-medium text-foreground shadow-md';

/** Floating drag chip shared by tab ghost and sidebar drag overlay. */
export const editorDragChipClass =
  'inline-flex h-[var(--titlebar-height)] shrink-0 cursor-grabbing items-center gap-2 rounded-none border border-border/60 bg-background px-3 text-xs font-medium text-foreground';
