import {
  colorSchemeDark,
  colorSchemeLight,
  type Theme,
  themeQuartz,
} from 'ag-grid-community';
import type { ThemeSettings } from '@/shared/types/settings';

export function buildAgGridTheme(settings: ThemeSettings): Theme {
  const isDark = settings.mode === 'dark';
  const gridBorder = isDark ? 'rgba(255, 255, 255, 0.14)' : settings.gridLines;
  const foreground = isDark ? '#fafafa' : '#0f172a';
  const subtleForeground = isDark ? '#a3a3a3' : '#64748b';

  return themeQuartz
    .withPart(isDark ? colorSchemeDark : colorSchemeLight)
    .withParams({
      accentColor: settings.accentColor,
      backgroundColor: settings.workbenchBackground,
      borderColor: gridBorder,
      browserColorScheme: settings.mode,
      cellFontSize: 11,
      cellTextColor: foreground,
      chromeBackgroundColor: settings.sidebarBackground,
      columnBorder: true,
      dataBackgroundColor: settings.workbenchBackground,
      fontFamily: ['var(--font-sans)', 'ui-sans-serif', 'system-ui', 'sans-serif'],
      fontSize: 11,
      foregroundColor: foreground,
      headerBackgroundColor: settings.sidebarBackground,
      headerCellHoverBackgroundColor: isDark
        ? 'rgba(255, 255, 255, 0.06)'
        : 'rgba(0, 0, 0, 0.04)',
      headerColumnBorder: true,
      headerFontSize: 11,
      headerFontWeight: 600,
      headerRowBorder: true,
      headerTextColor: foreground,
      modalOverlayBackgroundColor: isDark
        ? 'rgba(0, 0, 0, 0.18)'
        : 'rgba(255, 255, 255, 0.28)',
      oddRowBackgroundColor: settings.workbenchBackground,
      pinnedColumnBorder: true,
      rowBorder: true,
      rowHoverColor: isDark ? 'rgba(255, 255, 255, 0.04)' : 'rgba(0, 0, 0, 0.025)',
      subtleTextColor: subtleForeground,
      textColor: foreground,
      wrapperBackgroundColor: settings.workbenchBackground,
      wrapperBorder: false,
      wrapperBorderRadius: 0,
    });
}
