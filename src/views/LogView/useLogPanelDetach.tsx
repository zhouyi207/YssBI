import { useCallback } from 'react';
import { useTranslation } from 'react-i18next';
import { readEditorPartOptions } from '@/features/core/layout/editorPartOptions';
import { useDragToOpenWindow } from '@/features/core/workbench/dragToOpenWindow';
import { openLogsWindow } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import type { LogPanelVariant } from './useLogPanelController';
import type { DragToOpenWindowHandleProps } from '@/features/core/workbench/dragToOpenWindow';

export function useLogPanelDetach(variant: LogPanelVariant): {
  dragHandleRef: ReturnType<typeof useDragToOpenWindow>['dragHandleRef'];
  dragHandleProps: DragToOpenWindowHandleProps | null;
} {
  const { t } = useTranslation();
  const embedded = variant === 'embedded';

  const handleOpenAuxiliaryWindow = useCallback(async (bounds: { x: number; y: number }) => {
    try {
      await openLogsWindow({ fallbackX: bounds.x, fallbackY: bounds.y });
    } catch (err) {
      logger.app.error(`Failed to open logs window: ${String(err)}`, 'LogPanel');
    }
  }, []);

  return useDragToOpenWindow({
    enabled: embedded,
    dragPayload: { kind: 'logs-panel' },
    dragImageLabel: t('log.title'),
    dragToOpenWindowEnabled: readEditorPartOptions().dragToOpenWindow,
    dragSurface: 'header-row',
    onOpenAuxiliaryWindow: handleOpenAuxiliaryWindow,
  });
}
