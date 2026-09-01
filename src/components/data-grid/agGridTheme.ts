import {
  colorSchemeDark,
  colorSchemeLight,
  type Theme,
  type ThemeDefaultParams,
  themeQuartz,
} from "ag-grid-community";
import type { ThemeSettings } from "@/shared/types/settings";
import { resolveThemeTokens } from "@/shared/theme/themeTokens";

export function getAgGridThemeParams(settings: ThemeSettings): Partial<ThemeDefaultParams> {
  const tokens = resolveThemeTokens(settings);

  return {
    accentColor: tokens.accent,
    backgroundColor: tokens.workbenchBg,
    borderColor: tokens.border,
    browserColorScheme: settings.mode,
    cellFontSize: 11,
    cellTextColor: tokens.foreground,
    chromeBackgroundColor: tokens.sidebarBg,
    columnBorder: true,
    dataBackgroundColor: tokens.workbenchBg,
    fontFamily: ["var(--font-sans)", "ui-sans-serif", "system-ui", "sans-serif"],
    fontSize: 11,
    foregroundColor: tokens.foreground,
    headerBackgroundColor: tokens.panelHeaderBg,
    headerCellHoverBackgroundColor: tokens.accentSoft,
    headerColumnBorder: true,
    headerFontSize: 11,
    headerFontWeight: 600,
    headerRowBorder: true,
    headerTextColor: tokens.foreground,
    modalOverlayBackgroundColor: tokens.surfaceSunken,
    oddRowBackgroundColor: tokens.workbenchBg,
    pinnedColumnBorder: true,
    rowBorder: true,
    rowHoverColor: tokens.accentSoft,
    subtleTextColor: tokens.mutedForeground,
    textColor: tokens.foreground,
    wrapperBackgroundColor: tokens.workbenchBg,
    wrapperBorder: false,
    wrapperBorderRadius: 0,
  };
}

export function buildAgGridTheme(settings: ThemeSettings): Theme {
  return themeQuartz
    .withPart(settings.mode === "dark" ? colorSchemeDark : colorSchemeLight)
    .withParams(getAgGridThemeParams(settings));
}
