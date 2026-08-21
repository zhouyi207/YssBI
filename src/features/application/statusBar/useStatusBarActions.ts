import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { editorDockviewPort } from '@/features/core/dockview';
import { showPanelView } from '@/features/core/layout/workbenchLayoutService';
import { setViewportLive, editorViewportScope } from '@/features/core/viewport';

/** Bottom bar command handlers — keeps BottomBar presentational. */
export function useStatusBarActions() {
  const { t } = useTranslation();

  const openLogsPanel = useCallback(() => {
    showPanelView('logs');
  }, []);

  const resetCanvasViewport = useCallback(() => {
    const panel = editorDockviewPort.getActivePanel();
    const value = panel?.tab?.data?.layoutTab;
    if (!panel || !value || typeof value !== 'object') return;
    const graphPath = (value as { id?: unknown }).id;
    if (typeof graphPath !== 'string') return;
    setViewportLive(editorViewportScope(panel.groupId, graphPath), { ...DEFAULT_VIEWPORT });
  }, []);


  return {
    openLogsPanel,
    resetCanvasViewport,
    executionTooltip: t('bottomBar.openLogsPanel'),
    viewportTooltip: t('bottomBar.resetViewport'),
  };
}
