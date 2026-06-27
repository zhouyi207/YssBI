/** Shared Tailwind classes for Detail panel sections (shadcn tokens). */

export const detailTableClass = 'text-sm text-foreground';

export const detailLabelCellClass = 'text-xs font-medium text-muted-foreground';

export const detailLabelCellNarrowClass = `${detailLabelCellClass} w-20`;

export const detailLabelCellWideClass = `${detailLabelCellClass} w-24`;

export const detailValueMutedClass = 'text-muted-foreground';

export const detailBodyTextClass = 'text-sm leading-relaxed text-foreground';

export const detailMetaTextClass = 'text-xs leading-relaxed text-muted-foreground';

export const detailSmallMetaTextClass = 'text-[11px] leading-snug text-muted-foreground/80';

export const detailMonoTextClass = 'font-mono text-xs leading-relaxed text-muted-foreground';

export const detailAccentMonoTextClass = 'font-mono text-xs leading-relaxed text-[var(--accent-color)]/85';

export const detailSectionTitleClass =
  'text-xs font-semibold uppercase tracking-wide text-muted-foreground';

export const detailSubsectionTitleClass =
  'text-xs font-semibold uppercase tracking-wide text-muted-foreground';

export const detailPinRowClass =
  'group flex items-center gap-2 rounded-lg border border-border/60 bg-card px-2.5 py-2 shadow-xs transition-colors hover:bg-accent/30';

export const detailBadgeClass =
  'border-border bg-secondary text-secondary-foreground';

export const detailEmptyHintClass = 'rounded-lg border border-dashed border-border px-3 py-2 text-center text-xs italic text-muted-foreground';

export const detailListItemClass =
  'flex items-center justify-between rounded-lg border border-border/60 bg-card px-3 py-2 text-sm text-foreground shadow-xs transition-colors hover:bg-accent/30';

export const detailInlineInputClass =
  'h-8 bg-background/60 text-sm font-medium';

export const detailInlineInputSmallClass =
  'h-8 flex-1 bg-background/60 text-xs shadow-none';

export const detailNestedScrollClass = 'max-h-44 rounded-lg border border-border/60 bg-card';

export const detailNestedTableClass = 'text-xs text-foreground';

export const detailNestedTableHeadClass = 'h-8 bg-muted/40 px-3 py-2 text-xs font-medium uppercase tracking-wide';

export const detailProseClass =
  'prose prose-invert max-w-none text-sm leading-relaxed text-foreground [&_h1]:text-base [&_h1]:font-bold [&_h2]:text-sm [&_h2]:font-semibold [&_h3]:text-xs [&_p]:my-2 [&_table]:text-xs [&_td]:border [&_td]:border-border [&_td]:px-2 [&_td]:py-1 [&_th]:border [&_th]:border-border [&_th]:px-2 [&_th]:py-1 [&_ul]:my-2 [&_ul]:list-disc [&_ul]:pl-4 [&_.katex]:text-foreground';
