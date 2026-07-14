import { useCallback, useEffect, useMemo, useRef } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { LOGS_DRAG_TYPE } from '@/app/appConfig/default';
import { openLogsWindow } from '@/features/application/window';
import { logger } from '@/utils/appLogger';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';
import type { LogPanelVariant } from './useLogPanelController';
import type { HTMLAttributes } from 'react';

export type LogPanelHeaderDragProps = Pick<
  HTMLAttributes<HTMLElement>,
  'draggable' | 'onDragStart' | 'onDragEnd' | 'title'
>;

export function useLogPanelDetach(variant: LogPanelVariant): {
  leadingDragProps: LogPanelHeaderDragProps | null;
  dragPreviewPortal: React.ReactNode;
} {
  const { t } = useTranslation();
  const dragImageRef = useRef<HTMLDivElement>(null);
  const droppedOnOurWindowRef = useRef(false);
  const lastDragPosRef = useRef<{ x: number; y: number } | null>(null);

  const openInNewWindow = useCallback(async (x?: number, y?: number) => {
    await openLogsWindow({
      fallbackX: typeof x === 'number' ? x : undefined,
      fallbackY: typeof y === 'number' ? y : undefined,
    });
  }, []);

  const handleEmbeddedDragStart = useCallback((e: React.DragEvent) => {
    if (variant !== 'embedded') return;
    droppedOnOurWindowRef.current = false;
    lastDragPosRef.current = { x: e.screenX, y: e.screenY };
    e.dataTransfer.setData(LOGS_DRAG_TYPE, '');
    e.dataTransfer.effectAllowed = 'move';
    if (dragImageRef.current) {
      e.dataTransfer.setDragImage(dragImageRef.current, 0, 0);
    }
  }, [variant]);

  const handleEmbeddedDragEnd = useCallback(async (e: React.DragEvent) => {
    if (variant !== 'embedded') return;
    const last = lastDragPosRef.current;
    lastDragPosRef.current = null;
    if (!droppedOnOurWindowRef.current) {
      const sx = e.screenX ?? 0;
      const sy = e.screenY ?? 0;
      const pos = (sx !== 0 || sy !== 0) ? { x: sx, y: sy } : (last ?? { x: 100, y: 100 });
      try {
        await openInNewWindow(pos.x, pos.y);
      } catch (err) {
        logger.app.error('Failed to open logs window: ' + String(err), 'LogPanel');
      }
    }
  }, [variant, openInNewWindow]);

  useEffect(() => {
    if (variant !== 'embedded') return;
    const onDragOver = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes(LOGS_DRAG_TYPE)) {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
        lastDragPosRef.current = { x: e.screenX, y: e.screenY };
      }
    };
    const onDrop = (e: DragEvent) => {
      if (e.dataTransfer?.types.includes(LOGS_DRAG_TYPE)) {
        e.preventDefault();
        droppedOnOurWindowRef.current = true;
      }
    };
    const cleanupDragOver = addGlobalEventListener(document, 'dragover', onDragOver);
    const cleanupDrop = addGlobalEventListener(document, 'drop', onDrop);
    return () => {
      cleanupDragOver();
      cleanupDrop();
    };
  }, [variant]);

  const leadingDragProps: LogPanelHeaderDragProps | null = useMemo(
    () => (variant === 'embedded'
      ? {
          draggable: true,
          onDragStart: handleEmbeddedDragStart,
          onDragEnd: handleEmbeddedDragEnd,
          title: t('log.dragToOpenWindow'),
        }
      : null),
    [variant, handleEmbeddedDragStart, handleEmbeddedDragEnd, t],
  );

  const dragPreviewPortal = variant === 'embedded'
    ? createPortal(
        <div
          ref={dragImageRef}
          className="pointer-events-none fixed -left-[9999px] -top-[9999px] w-64 select-none overflow-hidden rounded-lg border border-[var(--accent-color)]/40 bg-[var(--workbench-bg)] opacity-95 shadow-xl"
          aria-hidden
        >
          <div className="flex items-center gap-2 border-b border-border/60 bg-[var(--sidebar-bg)] px-3 py-2">
            <span className="text-xs font-medium text-foreground">{t('log.title')}</span>
            <span className="text-[10px] text-muted-foreground">{t('log.releaseToCreateWindow')}</span>
          </div>
        </div>,
        document.body,
      )
    : null;

  return { leadingDragProps, dragPreviewPortal };
}
