import { useMemo } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { resolveThemeTokens, type ResolvedThemeTokens } from './themeTokens';

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
  const theme = useSettingsStore((s) => s.theme);
  return useMemo(() => getChartThemeColors(resolveThemeTokens(theme)), [theme]);
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
  const theme = useSettingsStore((s) => s.theme);
  return useMemo(() => getChartSeriesColors(resolveThemeTokens(theme)), [theme]);
}
