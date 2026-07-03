import type { Theme } from '@glideapps/glide-data-grid';
import type { ThemeSettings } from '@/shared/types/settings';

/**
 * 将应用设置里的主题色映射为 Glide `DataEditor` 的 `theme` 覆盖层（内部会与 `getDataEditorTheme()` 合并）。
 */
export function buildDataGridThemeOverlay(settings: ThemeSettings): Partial<Theme> {
  const isDark = settings.mode === 'dark';
  /** 深色默认 `gridLines` 与 `sidebarBackground` 几乎同色，格线/表头底线在画布上会消失 */
  const gridBorder = isDark ? 'rgba(255, 255, 255, 0.14)' : settings.gridLines;
  return {
    accentColor: settings.accentColor,
    accentFg: '#ffffff',
    accentLight: isDark ? 'rgba(255,255,255,0.08)' : 'rgba(37, 99, 235, 0.12)',
    bgCell: settings.workbenchBackground,
    bgCellMedium: settings.sidebarBackground,
    bgHeader: settings.sidebarBackground,
    bgHeaderHovered: isDark ? 'rgba(255,255,255,0.06)' : 'rgba(0,0,0,0.04)',
    bgHeaderHasFocus: settings.accentColor,
    bgBubble: settings.sidebarBackground,
    bgBubbleSelected: settings.accentColor,
    textDark: isDark ? '#fafafa' : '#0f172a',
    textMedium: isDark ? '#a3a3a3' : '#64748b',
    textLight: isDark ? '#737373' : '#94a3b8',
    textBubble: isDark ? '#fafafa' : '#0f172a',
    textHeader: isDark ? '#fafafa' : '#0f172a',
    textHeaderSelected: '#ffffff',
    bgIconHeader: settings.sidebarBackground,
    fgIconHeader: isDark ? '#a3a3a3' : '#64748b',
    borderColor: gridBorder,
    horizontalBorderColor: gridBorder,
    headerBottomBorderColor: gridBorder,
    linkColor: settings.accentColor,
    fontFamily: 'var(--font-sans), ui-sans-serif, system-ui, sans-serif',
    baseFontStyle: '11px var(--font-sans), ui-sans-serif, system-ui, sans-serif',
    headerFontStyle: '600 11px var(--font-sans), ui-sans-serif, system-ui, sans-serif',
    markerFontStyle: '11px var(--font-sans), ui-sans-serif, system-ui, sans-serif',
    editorFontSize: '11px',
  };
}

/** 行号列与表头侧栏背景一致 */
export function buildRowMarkerThemeOverlay(settings: ThemeSettings): Partial<Theme> {
  const overlay = buildDataGridThemeOverlay(settings);
  return {
    bgCell: settings.sidebarBackground,
    bgCellMedium: settings.sidebarBackground,
    bgHeader: settings.sidebarBackground,
    bgHeaderHovered: overlay.bgHeaderHovered,
    bgHeaderHasFocus: settings.accentColor,
    accentLight: overlay.accentLight,
    textLight: overlay.textDark,
    textMedium: overlay.textMedium,
  };
}
