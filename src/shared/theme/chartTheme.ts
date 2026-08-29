import { useMemo, useSyncExternalStore } from 'react';
import type { ResolvedThemeTokens } from './themeTokens';

export const THEME_TOKENS_CHANGED_EVENT = 'yssbi-theme-tokens-changed';

function cssVariable(name: string): string {
  return `var(${name})`;
}

function subscribeThemeTokens(listener: () => void): () => void {
  if (typeof window === 'undefined') return () => undefined;
  window.addEventListener(THEME_TOKENS_CHANGED_EVENT, listener);
  return () => window.removeEventListener(THEME_TOKENS_CHANGED_EVENT, listener);
}

function themeTokensSnapshot(): string {
  if (typeof document === 'undefined') return '';
  return document.documentElement.getAttribute('style') ?? '';
}

function cssChartThemeColors(): ChartThemeColors {
  return {
    canvas: cssVariable('--panel-bg'),
    grid: cssVariable('--grid-lines'),
    axis: cssVariable('--strong-border'),
    tick: cssVariable('--text-secondary'),
    label: cssVariable('--text-secondary'),
    zeroLine: cssVariable('--connection-lines'),
    tooltipBg: cssVariable('--surface-raised'),
    tooltipFg: cssVariable('--text-primary'),
    tooltipMuted: cssVariable('--text-secondary'),
  };
}

function cssChartSeriesColors(): ChartSeriesColors {
  return {
    primary: cssVariable('--accent-color'),
    negative: cssVariable('--status-danger'),
    secondary: cssVariable('--status-warning'),
    highlight: cssVariable('--status-danger'),
    palette: [
      cssVariable('--accent-color'),
      cssVariable('--status-danger'),
      cssVariable('--status-success'),
      cssVariable('--status-warning'),
      cssVariable('--status-info'),
      cssVariable('--pin-temporal'),
      cssVariable('--pin-table'),
    ],
  };
}

/** D3 / SVG 图表在 dark / light 下的配色（Summary、Plot 窗口共用） */
export interface ChartThemeColors {
  canvas: string;
  grid: string;
  axis: string;
  tick: string;
  label: string;
  zeroLine: string;
  tooltipBg: string;
  tooltipFg: string;
  tooltipMuted: string;
}

export function getChartThemeColors(tokens: ResolvedThemeTokens): ChartThemeColors {
  return {
    canvas: tokens.panelBg,
    grid: tokens.grid,
    axis: tokens.border,
    tick: tokens.mutedForeground,
    label: tokens.mutedForeground,
    zeroLine: tokens.connection,
    tooltipBg: tokens.surfaceRaised,
    tooltipFg: tokens.foreground,
    tooltipMuted: tokens.mutedForeground,
  };
}

export function useChartThemeColors(): ChartThemeColors {
  const style = useSyncExternalStore(
    subscribeThemeTokens,
    themeTokensSnapshot,
    themeTokensSnapshot,
  );
  return useMemo(() => cssChartThemeColors(), [style]);
}

/** D3 序列色：主色跟随主题 accent，其余为固定语义色 */
export interface ChartSeriesColors {
  primary: string;
  negative: string;
  secondary: string;
  highlight: string;
  palette: string[];
}

export function getChartSeriesColors(tokens: ResolvedThemeTokens): ChartSeriesColors {
  return {
    primary: tokens.accent,
    negative: tokens.status.danger,
    secondary: tokens.status.warning,
    highlight: tokens.status.danger,
    palette: [
      tokens.accent,
      tokens.status.danger,
      tokens.status.success,
      tokens.status.warning,
      tokens.status.info,
      tokens.pins.temporal,
      tokens.pins.table,
    ],
  };
}

export function useChartSeriesColors(): ChartSeriesColors {
  const style = useSyncExternalStore(
    subscribeThemeTokens,
    themeTokensSnapshot,
    themeTokensSnapshot,
  );
  return useMemo(() => cssChartSeriesColors(), [style]);
}
