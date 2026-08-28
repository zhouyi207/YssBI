import { createContext, useContext, type ReactNode } from 'react';
import type {
  ChartSeriesColors,
  ChartThemeColors,
} from '@/shared/theme/chartTheme';

export interface ChartThemeValue {
  colors: ChartThemeColors;
  series: ChartSeriesColors;
}

const ChartThemeContext = createContext<ChartThemeValue | null>(null);

export function ChartThemeContextProvider({
  children,
  value,
}: {
  children: ReactNode;
  value: ChartThemeValue;
}) {
  return (
    <ChartThemeContext.Provider value={value}>
      {children}
    </ChartThemeContext.Provider>
  );
}

export function useChartTheme(): ChartThemeValue {
  const value = useContext(ChartThemeContext);
  if (!value) {
    throw new Error('useChartTheme must be used within ChartThemeProvider');
  }
  return value;
}
