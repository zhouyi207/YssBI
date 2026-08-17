import { useCallback, useLayoutEffect, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { LOG_ITEM_GAP, LOG_ITEM_HEIGHT } from '@/app/appConfig/default';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { isLogViewportPinnedToBottom } from './logPanelScroll';
import { snapLogViewportToBottom } from './logPanelViewport';
import type { LogPanelVariant } from './useLogPanelController';

interface UseLogPanelVirtualListOptions {
  logs: DiagnosticRecordDto[];
  autoScroll: boolean;
  variant: LogPanelVariant;
  refreshScrollToken: number;
}

export function useLogPanelVirtualList({
  logs,
  autoScroll,
  variant,
  refreshScrollToken,
}: UseLogPanelVirtualListOptions) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const pinnedToBottomRef = useRef(true);
  const logsRef = useRef(logs);
  logsRef.current = logs;

  const virtualizer = useVirtualizer({
    count: logs.length,
    getScrollElement: () => viewportRef.current,
    estimateSize: () => LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
    overscan: 8,
  });

  const snapToBottom = useCallback(() => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    snapLogViewportToBottom(viewport, logsRef.current.length);
    pinnedToBottomRef.current = true;
  }, []);

  useLayoutEffect(() => {
    snapToBottom();
  }, [snapToBottom]);

  useLayoutEffect(() => {
    if (!autoScroll || !pinnedToBottomRef.current) return;
    snapToBottom();
  }, [logs, autoScroll, snapToBottom]);

  useLayoutEffect(() => {
    if (!autoScroll) {
      pinnedToBottomRef.current = false;
      return;
    }
    snapToBottom();
  }, [autoScroll, snapToBottom]);

  useLayoutEffect(() => {
    if (autoScroll) snapToBottom();
  }, [refreshScrollToken, autoScroll, snapToBottom]);

  const handleScroll = useCallback((event: React.UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = event.currentTarget;
    pinnedToBottomRef.current = isLogViewportPinnedToBottom(
      scrollTop,
      scrollHeight,
      clientHeight,
    );
  }, []);

  useLayoutEffect(() => {
    const viewport = viewportRef.current;
    if (!viewport || variant !== 'embedded') return;
    const observer = new ResizeObserver(() => {
      virtualizer.measure();
      if (autoScroll && pinnedToBottomRef.current) snapToBottom();
    });
    observer.observe(viewport);
    return () => observer.disconnect();
  }, [autoScroll, snapToBottom, variant, virtualizer]);

  return { viewportRef, virtualizer, handleScroll };
}
