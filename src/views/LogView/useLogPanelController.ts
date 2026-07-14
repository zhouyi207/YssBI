import { useEffect, useRef, useState, useCallback, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useVirtualizer } from '@tanstack/react-virtual';
import { LOG_ITEM_HEIGHT, LOG_ITEM_GAP } from '@/app/appConfig/default';
import { useLogStore, applyLogFilter } from '@/features/core/log/logStore';
import { logBuffer } from '@/features/core/log/logBuffer';
import { useLiveLogs } from '@/features/core/log/useLiveLogs';
import { useLogActions } from '@/features/application/log';
import type { LogMessage } from '@/shared/types/ui';
import { togglePanelVisibility } from '@/features/core/layout/workbenchLayoutService';
import { useEditorStore } from '@/features/core/editor';
import { usePartResizeCommit } from '@/features/application/layout/usePartResizeCommit';
import { useLogPanelDetach } from './useLogPanelDetach';

const SCROLL_BOTTOM_THRESHOLD = 80;

export type LogPanelVariant = 'embedded' | 'standalone';

export interface LogPanelController {
  variant: LogPanelVariant;
  logs: LogMessage[];
  filteredLogs: LogMessage[];
  total: number;
  loading: boolean;
  filter: ReturnType<typeof useLogStore.getState>['filter'];
  isFilterOpen: boolean;
  setIsFilterOpen: (open: boolean) => void;
  autoScroll: boolean;
  setAutoScroll: (next: boolean) => void;
  isInitialLoad: boolean;
  selectedIndex: number | null;
  filterButtonRef: React.RefObject<HTMLButtonElement | null>;
  filterPopoverRef: React.RefObject<HTMLDivElement | null>;
  popoverPosition: { top: number; left: number };
  logContainerRef: React.RefObject<HTMLDivElement | null>;
  virtualizer: ReturnType<typeof useVirtualizer<HTMLDivElement, Element>>;
  toggleLevel: ReturnType<typeof useLogStore.getState>['toggleLevel'];
  setSearchText: ReturnType<typeof useLogStore.getState>['setSearchText'];
  refreshLogs: () => void;
  clearLogs: () => void;
  handleClose: () => void;
  handleSelectLog: (index: number) => void;
  handleScroll: (e: React.UIEvent<HTMLDivElement>) => void;
  dragHandleRef: ReturnType<typeof useLogPanelDetach>['dragHandleRef'];
  dragHandleProps: ReturnType<typeof useLogPanelDetach>['dragHandleProps'];
}

export function useLogPanelController(variant: LogPanelVariant): LogPanelController {
  const { entries: logs, total, hasMore, loading } = useLiveLogs();
  const filter = useLogStore((s) => s.filter);
  const toggleLevel = useLogStore((s) => s.toggleLevel);
  const setSearchText = useLogStore((s) => s.setSearchText);
  const selectedLog = useLogStore((s) => s.selectedLog);
  const setSelectedLog = useLogStore((s) => s.setSelectedLog);
  const { loadLogs, loadMoreLogs, refreshLogs } = useLogActions();

  const [isFilterOpen, setIsFilterOpen] = useState(false);
  const [autoScroll, setAutoScroll] = useState(true);
  const [isInitialLoad, setIsInitialLoad] = useState(true);
  const [popoverPosition, setPopoverPosition] = useState({ top: 0, left: 0 });

  const pinnedToBottomRef = useRef(true);
  const logContainerRef = useRef<HTMLDivElement>(null);
  const filterButtonRef = useRef<HTMLButtonElement>(null);
  const filterPopoverRef = useRef<HTMLDivElement>(null);
  const loadMoreStateRef = useRef({ hasMore: false, loading, loadMoreLogs });

  const { dragHandleRef, dragHandleProps } = useLogPanelDetach(variant);

  const filteredLogs = useMemo(() => applyLogFilter(logs, filter), [logs, filter]);
  const selectedIndex = selectedLog
    ? filteredLogs.findIndex((l) => l === selectedLog)
    : null;

  const virtualizer = useVirtualizer({
    count: filteredLogs.length,
    getScrollElement: () => logContainerRef.current,
    estimateSize: () => LOG_ITEM_HEIGHT + LOG_ITEM_GAP,
    overscan: 8,
  });

  const virtualizerRef = useRef(virtualizer);
  virtualizerRef.current = virtualizer;

  loadMoreStateRef.current = { hasMore, loading, loadMoreLogs };

  useEffect(() => {
    loadLogs(0, 100).then(() => {
      setIsInitialLoad(false);
      setTimeout(() => {
        if (logContainerRef.current) {
          logContainerRef.current.scrollTop = logContainerRef.current.scrollHeight;
        }
      }, 100);
    });

    const unlisten = listen<LogMessage>('log-message', (event) => {
      logBuffer.pushLive(event.payload);
    });

    return () => {
      unlisten.then((fn) => fn());
    };
  }, [loadLogs]);

  useEffect(() => {
    const el = logContainerRef.current;
    if (!autoScroll || !el || !pinnedToBottomRef.current) return;
    el.scrollTop = el.scrollHeight;
  }, [logs, autoScroll]);

  useEffect(() => {
    if (!isFilterOpen || !filterButtonRef.current) return;
    const rect = filterButtonRef.current.getBoundingClientRect();
    setPopoverPosition({ top: rect.bottom + 4, left: rect.right - 280 });
    const handleClickOutside = (e: MouseEvent) => {
      const target = e.target as Node;
      if (
        filterButtonRef.current?.contains(target)
        || filterPopoverRef.current?.contains(target)
      ) return;
      setIsFilterOpen(false);
    };
    document.addEventListener('mousedown', handleClickOutside);
    return () => document.removeEventListener('mousedown', handleClickOutside);
  }, [isFilterOpen]);

  useEffect(() => {
    if (isInitialLoad) return;
    const el = logContainerRef.current;
    if (!el) return;

    const onScroll = () => {
      const { scrollTop, scrollHeight, clientHeight } = el;
      pinnedToBottomRef.current = scrollHeight - scrollTop - clientHeight < SCROLL_BOTTOM_THRESHOLD;
      const { hasMore, loading: isLoading, loadMoreLogs: loadMore } = loadMoreStateRef.current;
      if (el.scrollTop < 150 && hasMore && !isLoading) {
        const prevScrollHeight = el.scrollHeight;
        const prevScrollTop = el.scrollTop;
        loadMore().then(() => {
          if (!logContainerRef.current) return;
          const heightDiff = logContainerRef.current.scrollHeight - prevScrollHeight;
          logContainerRef.current.scrollTop = prevScrollTop + heightDiff;
        });
      }
    };

    const id = requestAnimationFrame(() => {
      el.addEventListener('scroll', onScroll, { passive: true });
    });
    return () => {
      cancelAnimationFrame(id);
      el.removeEventListener('scroll', onScroll);
    };
  }, [isInitialLoad]);

  const tryLoadOlder = useCallback(() => {
    const { hasMore, loading: isLoading, loadMoreLogs: loadMore } = loadMoreStateRef.current;
    if (!hasMore || isLoading || !logContainerRef.current) return;
    const el = logContainerRef.current;
    const prevScrollHeight = el.scrollHeight;
    const prevScrollTop = el.scrollTop;
    loadMore().then(() => {
      if (!logContainerRef.current) return;
      const heightDiff = logContainerRef.current.scrollHeight - prevScrollHeight;
      logContainerRef.current.scrollTop = prevScrollTop + heightDiff;
    });
  }, []);

  const handleScroll = useCallback((e: React.UIEvent<HTMLDivElement>) => {
    const target = e.currentTarget;
    const { scrollTop, scrollHeight, clientHeight } = target;
    pinnedToBottomRef.current = scrollHeight - scrollTop - clientHeight < SCROLL_BOTTOM_THRESHOLD;
    if (scrollTop < 150) tryLoadOlder();
  }, [tryLoadOlder]);

  const handleClose = useCallback(() => {
    if (variant === 'embedded') {
      togglePanelVisibility();
    } else {
      void getCurrentWindow().close();
    }
  }, [variant]);

  const handleSelectLog = useCallback((index: number) => {
    const log = filteredLogs[index] ?? null;
    setSelectedLog(log);
    if (log) {
      useEditorStore.getState().setDetailFocus({ kind: 'log' });
    } else {
      useEditorStore.getState().clearDetailFocus();
    }
  }, [filteredLogs, setSelectedLog]);

  const clearLogs = useCallback(() => {
    logBuffer.clear();
    setSelectedLog(null);
  }, [setSelectedLog]);

  const handleAutoScrollToggle = useCallback((next: boolean) => {
    setAutoScroll(next);
    if (next) {
      pinnedToBottomRef.current = true;
      const el = logContainerRef.current;
      if (el) el.scrollTop = el.scrollHeight;
    }
  }, []);

  usePartResizeCommit('panel', useCallback(() => {
    if (variant !== 'embedded') return;
    virtualizerRef.current.measure();
  }, [variant]));

  return {
    variant,
    logs,
    filteredLogs,
    total,
    loading,
    filter,
    isFilterOpen,
    setIsFilterOpen,
    autoScroll,
    setAutoScroll: handleAutoScrollToggle,
    isInitialLoad,
    selectedIndex,
    filterButtonRef,
    filterPopoverRef,
    popoverPosition,
    logContainerRef,
    virtualizer,
    toggleLevel,
    setSearchText,
    refreshLogs,
    clearLogs,
    handleClose,
    handleSelectLog,
    handleScroll,
    dragHandleRef,
    dragHandleProps,
  };
}
