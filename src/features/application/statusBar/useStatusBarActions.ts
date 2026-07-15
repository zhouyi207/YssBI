import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { getActiveLayoutTab, resolveEditorTargetGroupId } from '@/features/core/layout/layoutTabQueries';
import { togglePanelVisibility, setPanelActiveView } from '@/features/core/layout/workbenchLayoutService';
import { setViewportLive, editorViewportScope } from '@/features/core/viewport';
import { useSettingsStore } from '@/features/core/settings/settingsStore';
import { syncColorThemePreset } from '@/features/application/settings/appearanceRuntime';
import { getNextColorThemePreset } from '@/features/application/settings/colorThemePresets';

/** Bottom bar command handlers — keeps BottomBar presentational. */
export function useStatusBarActions() {
  const { t } = useTranslation();
  const updateTheme = useSettingsStore((s) => s.updateTheme);
  const colorTheme = useSettingsStore((s) => s.appearance.colorTheme);

  const openLogsPanel = useCallback(() => {
    const panel = useLayoutStore.getState().nodes.panel;
    if (panel?.data?.visible === false) {
      togglePanelVisibility();
    }
    setPanelActiveView('logs');
  }, []);

  const resetCanvasViewport = useCallback(() => {
    const state = useLayoutStore.getState();
    const groupId = resolveEditorTargetGroupId(undefined, state.nodes, state);
    const graphPath = getActiveLayoutTab(groupId, state.nodes)?.activeTabId ?? null;
    if (!graphPath) return;
    setViewportLive(editorViewportScope(groupId, graphPath), { ...DEFAULT_VIEWPORT });
  }, []);

  const cycleColorTheme = useCallback(() => {
    const next = getNextColorThemePreset(colorTheme);
    useSettingsStore.getState().updateAppearance({ colorTheme: next });
    syncColorThemePreset(next, updateTheme);
  }, [colorTheme, updateTheme]);

  return {
    openLogsPanel,
    resetCanvasViewport,
    cycleColorTheme,
    executionTooltip: t('bottomBar.openLogsPanel'),
    themeTooltip: t('bottomBar.cycleTheme'),
    viewportTooltip: t('bottomBar.resetViewport'),
  };
}
