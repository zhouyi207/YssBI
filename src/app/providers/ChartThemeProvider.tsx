import { useMemo, type ReactNode } from 'react';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { ChartThemeContextProvider } from '@/shared/charts/core/theme';
import {
  getChartSeriesColors,
  getChartThemeColors,
} from '@/shared/theme/chartTheme';
import { resolveThemeTokens } from '@/shared/theme/themeTokens';

export function ChartThemeProvider({ children }: { children: ReactNode }) {
  const theme = useSettingsStore((state) => state.theme);
  const value = useMemo(() => {
    const tokens = resolveThemeTokens(theme);
    return {
      colors: getChartThemeColors(tokens),
      series: getChartSeriesColors(tokens),
    };
  }, [theme]);

  return (
    <ChartThemeContextProvider value={value}>
      {children}
    </ChartThemeContextProvider>
  );
}
