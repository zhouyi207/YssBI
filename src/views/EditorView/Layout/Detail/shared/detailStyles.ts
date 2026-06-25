/** Shared Tailwind classes for Detail panel tables and sections (shadcn tokens). */

export const detailTableClass = 'text-[11px] text-foreground';

export const detailLabelCellClass = 'bg-muted/50 font-bold text-muted-foreground';

export const detailLabelCellNarrowClass = `${detailLabelCellClass} w-20`;

export const detailLabelCellWideClass = `${detailLabelCellClass} w-24`;

export const detailValueMutedClass = 'text-muted-foreground';

export const detailSectionTitleClass =
  'text-[10px] font-black uppercase tracking-widest text-muted-foreground';

export const detailSubsectionTitleClass =
  'text-[10px] font-semibold uppercase text-muted-foreground';

export const detailPinRowClass = 'group flex items-center gap-1 rounded bg-muted/50 p-1';

export const detailEmptyHintClass = 'py-1 text-center text-[9px] italic text-muted-foreground';

export const detailListItemClass =
  'flex items-center justify-between rounded px-2 py-1 text-[11px] text-muted-foreground hover:bg-muted/50';

export const detailInlineInputClass =
  'h-7 border-0 bg-transparent px-0 py-0 font-medium shadow-none';

export const detailInlineInputSmallClass =
  'h-6 flex-1 border-0 bg-transparent px-1 py-0 text-[10px] shadow-none';

export const detailNestedScrollClass = 'max-h-40 bg-muted/30';

export const detailProseClass =
  'prose prose-invert max-w-none text-[11px] leading-relaxed text-foreground [&_h1]:text-sm [&_h1]:font-bold [&_h2]:text-xs [&_h2]:font-semibold [&_h3]:text-[11px] [&_p]:my-2 [&_table]:text-[10px] [&_td]:border [&_td]:border-border [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-border [&_th]:px-2 [&_th]:py-1 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-4 [&_.katex]:text-foreground';
