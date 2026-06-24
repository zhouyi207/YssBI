import { useMemo } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';

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

export function getChartThemeColors(isDark: boolean): ChartThemeColors {
  if (isDark) {
    return {
      canvas: '#13151a',
      grid: '#2a2d35',
      axis: '#3a3d45',
      tick: '#8b8f9a',
      label: '#6b7080',
      zeroLine: '#4a4d55',
      tooltipBg: '#1e2028',
      tooltipFg: '#e0e0e0',
      tooltipMuted: '#888888',
    };
  }
  return {
    canvas: '#ffffff',
    grid: '#e5e7eb',
    axis: '#cbd5e1',
    tick: '#64748b',
    label: '#475569',
    zeroLine: '#94a3b8',
    tooltipBg: '#ffffff',
    tooltipFg: '#111827',
    tooltipMuted: '#64748b',
  };
}

export function useChartThemeColors(): ChartThemeColors {
  const mode = useSettingsStore((s) => s.theme.mode);
  return useMemo(() => getChartThemeColors(mode !== 'light'), [mode]);
}
