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

/** 标准 XY 图边距（Line / Scatter / ECDF / KDE / Bar / Histogram） */
export const DEFAULT_PLOT_MARGIN = { top: 20, right: 24, bottom: 40, left: 56 } as const;

/** 紧凑模式边距 */
export const COMPACT_PLOT_MARGIN = { top: 4, right: 4, bottom: 4, left: 4 } as const;

/** 相关矩阵图边距 */
export const CORRELATION_PLOT_MARGIN = { top: 40, right: 24, bottom: 120, left: 120 } as const;

/** ACF / PACF 柱状图边距 */
export const CORRELOGRAM_MARGIN = { top: 28, right: 24, bottom: 36, left: 52 } as const;

/** 平行坐标图边距 */
export const PARALLEL_COORDINATES_MARGIN = { top: 28, right: 16, bottom: 12, left: 16 } as const;

export type PlotMargin = { top: number; right: number; bottom: number; left: number };

/** Flex 子项图表（如 ACF/PACF 并排列） */
export const plotFlexShellClass = cn(plotShellClass, 'relative w-full flex-1 min-h-0');
