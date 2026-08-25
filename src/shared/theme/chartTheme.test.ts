import { describe, expect, it } from 'vitest';
import { DEFAULT_DARK_THEME } from '@/app/appConfig/default';
import { resolveThemeTokens } from './themeTokens';
import { getChartSeriesColors, getChartThemeColors } from './chartTheme';

describe('chartTheme', () => {
  it('derives chart surfaces and typography from resolved semantic tokens', () => {
    const tokens = resolveThemeTokens({
      ...DEFAULT_DARK_THEME,
      workbenchBackground: '#101820',
      sidebarBackground: '#202a36',
      foreground: '#f8fafc',
      mutedForeground: '#a8b3c2',
      gridColor: '#314052',
      borderColor: '#405266',
      selectionColor: '#5eead4',
    });

    expect(getChartThemeColors(tokens)).toEqual({
      canvas: tokens.panelBg,
      grid: tokens.grid,
      axis: tokens.border,
      tick: tokens.mutedForeground,
      label: tokens.mutedForeground,
      zeroLine: tokens.connection,
      tooltipBg: tokens.surfaceRaised,
      tooltipFg: tokens.foreground,
      tooltipMuted: tokens.mutedForeground,
    });
  });

  it('uses the resolved accent for the primary series and fixed semantic statuses', () => {
    const tokens = resolveThemeTokens({ ...DEFAULT_DARK_THEME, accentColor: '#22c55e' });
    const series = getChartSeriesColors(tokens);

    expect(series.primary).toBe(tokens.accent);
    expect(series.negative).toBe(tokens.status.danger);
    expect(series.secondary).toBe(tokens.status.warning);
    expect(series.highlight).toBe(tokens.status.danger);
  });
});
