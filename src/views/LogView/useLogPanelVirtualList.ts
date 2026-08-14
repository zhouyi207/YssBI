import { useCallback, useLayoutEffect, useRef } from 'react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from '@/app/appConfig/default';

import type { LogMessage } from '@/shared/types/ui';
import {
  isLogViewportPinnedToBottom,
  shouldLoadOlderLogs,
} from './logPanelScroll';
import { snapLogViewportToBottom } from './logPanelViewport';
import type { LogPanelVariant } from './useLogPanelController';

interface UseLogPanelVirtualListOptions {
  logs: LogMessage[];
  autoScroll: boolean;
  hasMore: boolean;
  loading: boolean;
  loadMoreLogs: () => Promise<void>;
  variant: LogPanelVariant;
  refreshScrollToken: number;
}

export function useLogPanelVirtualList({
  logs,
  autoScroll,
  hasMore,
  loading,
  loadMoreLogs,
  variant,
  refreshScrollToken,
}: UseLogPanelVirtualListOptions) {
  const viewportRef = useRef<HTMLDivElement>(null);
  const pinnedToBottomRef = useRef(true);
  const logsRef = useRef(logs);
  logsRef.current = logs;

  const loadMoreStateRef = useRef({ hasMore, loading, loadMoreLogs });
  loadMoreStateRef.current = { hasMore, loading, loadMoreLogs };

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
    if (!autoScroll) return;
    snapToBottom();
  }, [refreshScrollToken, autoScroll, snapToBottom]);

  const preserveScrollWhilePrepending = useCallback((run: () => Promise<void>) => {
    const viewport = viewportRef.current;
    if (!viewport) return;
    const prevScrollHeight = viewport.scrollHeight;
    const prevScrollTop = viewport.scrollTop;
    void run().then(() => {
      if (!viewportRef.current) return;
      const heightDiff = viewportRef.current.scrollHeight - prevScrollHeight;
      const nextScrollTop = prevScrollTop + heightDiff;
      viewportRef.current.scrollTop = nextScrollTop;
      pinnedToBottomRef.current = isLogViewportPinnedToBottom(
        nextScrollTop,
        viewportRef.current.scrollHeight,
        viewportRef.current.clientHeight,
      );
    });
  }, []);

  const tryLoadOlder = useCallback(() => {
    const { hasMore: canLoadMore, loading: isLoading, loadMoreLogs: loadMore } = loadMoreStateRef.current;
    if (!canLoadMore || isLoading) return;
    preserveScrollWhilePrepending(loadMore);
  }, [preserveScrollWhilePrepending]);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const { scrollTop, scrollHeight, clientHeight } = e.currentTarget;
    pinnedToBottomRef.current = isLogViewportPinnedToBottom(scrollTop, scrollHeight, clientHeight);
    if (shouldLoadOlderLogs(scrollTop)) {
      tryLoadOlder();
    }
  }, [tryLoadOlder]);

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

  return {
    viewportRef,
    virtualizer,
    handleScroll,
  };
}
