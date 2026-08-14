import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { editorDockviewPort } from '@/features/core/dockview';
import { togglePanelVisibility, setPanelActiveView } from '@/features/core/layout/workbenchLayoutService';
import { setViewportLive, editorViewportScope } from '@/features/core/viewport';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { useWorkbenchStore } from '@/features/core/workbench';
import { syncColorThemePreset } from '@/features/application/settings/appearanceRuntime';
import { getNextColorThemePreset, getThemeModeForPreset } from '@/features/application/settings/colorThemePresets';

/** Bottom bar command handlers — keeps BottomBar presentational. */
export function useStatusBarActions() {
  const { t } = useTranslation();
  const updateTheme = useSettingsStore((s) => s.updateTheme);
  const appearance = useSettingsStore((s) => s.appearance);
  const colorTheme = appearance.colorTheme;

  const selectColorTheme = useCallback((next: string) => {
    const mode = getThemeModeForPreset(next);
    useSettingsStore.getState().updateAppearance({
      colorTheme: next,
      ...(mode === 'light' ? { lastLightColorTheme: next } : { lastDarkColorTheme: next }),
    });
    syncColorThemePreset(next, updateTheme);
  }, [updateTheme]);

  const openLogsPanel = useCallback(() => {
    if (useWorkbenchStore.getState().panelUserHidden) {
      togglePanelVisibility();
    }
    setPanelActiveView('logs');
  }, []);

  const resetCanvasViewport = useCallback(() => {
    const panel = editorDockviewPort.getActivePanel();
    const value = panel?.tab?.data?.layoutTab;
    if (!panel || !value || typeof value !== 'object') return;
    const graphPath = (value as { id?: unknown }).id;
    if (typeof graphPath !== 'string') return;
    setViewportLive(editorViewportScope(panel.groupId, graphPath), { ...DEFAULT_VIEWPORT });
  }, []);

  const cycleColorTheme = useCallback(() => {
    const next = getNextColorThemePreset(colorTheme);
    selectColorTheme(next);
  }, [colorTheme, selectColorTheme]);

  return {
    openLogsPanel,
    resetCanvasViewport,
    cycleColorTheme,
    executionTooltip: t('bottomBar.openLogsPanel'),
    themeTooltip: t('bottomBar.cycleTheme'),
    viewportTooltip: t('bottomBar.resetViewport'),
  };
}
