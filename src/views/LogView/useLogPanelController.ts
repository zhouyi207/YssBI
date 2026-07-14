import { useEffect, useState, useCallback, useMemo } from 'react';
import { listen } from '@tauri-apps/api/event';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useLogStore, applyLogFilter } from '@/features/core/log/logStore';
import { logBuffer } from '@/features/core/log/logBuffer';
import { useLiveLogs } from '@/features/core/log/useLiveLogs';
import { useLogActions } from '@/features/application/log';
import type { LogMessage } from '@/shared/types/ui';
import { togglePanelVisibility } from '@/features/core/layout/workbenchLayoutService';
import { useEditorStore } from '@/features/core/editor';
import { useLogPanelDetach } from './useLogPanelDetach';

export type LogPanelVariant = 'embedded' | 'standalone';

export interface LogPanelController {
  variant: LogPanelVariant;
  logs: LogMessage[];
  filteredLogs: LogMessage[];
  total: number;
  loading: boolean;
  hasMore: boolean;
  loadMoreLogs: () => Promise<void>;
  filter: ReturnType<typeof useLogStore.getState>['filter'];
  activeLogTypeTab: ReturnType<typeof useLogStore.getState>['activeLogTypeTab'];
  autoScroll: boolean;
  setAutoScroll: (next: boolean) => void;
  refreshScrollToken: number;
  isInitialLoad: boolean;
  selectedIndex: number | null;
  toggleLevel: ReturnType<typeof useLogStore.getState>['toggleLevel'];
  setSearchText: ReturnType<typeof useLogStore.getState>['setSearchText'];
  refreshLogs: () => void;
  clearLogs: () => void;
  handleClose: () => void;
  handleSelectLog: (index: number) => void;
  dragHandleRef: ReturnType<typeof useLogPanelDetach>['dragHandleRef'];
  dragHandleProps: ReturnType<typeof useLogPanelDetach>['dragHandleProps'];
}

export function useLogPanelController(variant: LogPanelVariant): LogPanelController {
  const { entries: logs, total, hasMore, loading } = useLiveLogs();
  const filter = useLogStore((s) => s.filter);
  const activeLogTypeTab = useLogStore((s) => s.activeLogTypeTab);
  const toggleLevel = useLogStore((s) => s.toggleLevel);
  const setSearchText = useLogStore((s) => s.setSearchText);
  const selectedLog = useLogStore((s) => s.selectedLog);
  const setSelectedLog = useLogStore((s) => s.setSelectedLog);
  const { loadLogs, loadMoreLogs, refreshLogs: refreshLogsAction } = useLogActions();

  const [autoScroll, setAutoScroll] = useState(true);
  const [isInitialLoad, setIsInitialLoad] = useState(true);
  const [refreshScrollToken, setRefreshScrollToken] = useState(0);

  const { dragHandleRef, dragHandleProps } = useLogPanelDetach(variant);

  const filteredLogs = useMemo(() => applyLogFilter(logs, filter), [logs, filter]);

  const selectedIndex = selectedLog
    ? filteredLogs.findIndex((l) => l === selectedLog)
    : null;

  useEffect(() => {
    void loadLogs(0, 100).then(() => {
      setIsInitialLoad(false);
    });

    const unlisten = listen<LogMessage>('log-message', (event) => {
      logBuffer.pushLive(event.payload);
    });

    return () => {
      void unlisten.then((fn) => fn());
    };
  }, [loadLogs]);

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
  }, []);

  const refreshLogs = useCallback(() => {
    void refreshLogsAction().then(() => {
      if (!autoScroll) return;
      setRefreshScrollToken((token) => token + 1);
    });
  }, [autoScroll, refreshLogsAction]);

  return {
    variant,
    logs,
    filteredLogs,
    total,
    loading,
    hasMore,
    loadMoreLogs,
    filter,
    activeLogTypeTab,
    autoScroll,
    setAutoScroll: handleAutoScrollToggle,
    refreshScrollToken,
    isInitialLoad,
    selectedIndex,
    toggleLevel,
    setSearchText,
    refreshLogs,
    clearLogs,
    handleClose,
    handleSelectLog,
    dragHandleRef,
    dragHandleProps,
  };
}
