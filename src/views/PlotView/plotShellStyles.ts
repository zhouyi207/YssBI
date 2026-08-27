import { cn } from '@/lib/utils';

/** Standalone plot window shell (border + card background). */
export const plotShellClass = 'rounded-lg border border-border bg-card overflow-hidden';

/** Worksheet / embedded preview: fill container, no chrome. */
export const plotShellEmbeddedClass =
  'relative h-full w-full min-h-0 overflow-hidden bg-[var(--workbench-bg)]';

export const plotToolbarClass =
  'flex items-center gap-4 px-3 py-1.5 border-b border-border bg-muted/20';

export const plotTooltipClass =
  'absolute pointer-events-none rounded px-2 py-1 bg-popover text-popover-foreground border border-border shadow-lg opacity-0 transition-opacity duration-100 z-10 whitespace-nowrap';

export const plotTooltipRichClass =
  'absolute pointer-events-none rounded px-2 py-1.5 bg-popover text-popover-foreground border border-border shadow-lg opacity-0 transition-opacity duration-100 z-10 text-[10px] leading-relaxed whitespace-nowrap';

export function plotContainerClass(embedded?: boolean, heightProp?: number | undefined): string {
  if (embedded) {
    return plotShellEmbeddedClass;
  }
  return cn(plotShellClass, !heightProp ? 'w-full h-full min-h-0' : 'relative');
}

/** Flex 子项图表（如 ACF/PACF 并排列） */
export const plotFlexShellClass = cn(plotShellClass, 'relative w-full flex-1 min-h-0');
