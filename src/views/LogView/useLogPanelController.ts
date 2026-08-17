import { useCallback, useMemo, useState } from 'react';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useDiagnosticSubscription } from '@/features/application/log';
import { useEditorStore } from '@/features/core/editor';
import { logBuffer } from '@/features/core/log/logBuffer';
import { applyLogFilter, useLogStore } from '@/features/core/log/logStore';
import { useLiveLogs } from '@/features/core/log/useLiveLogs';
import { togglePanelCollapsed } from '@/features/core/layout/workbenchLayoutService';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { useLogPanelDetach } from './useLogPanelDetach';

export type LogPanelVariant = 'embedded' | 'standalone';

export interface LogPanelController {
  variant: LogPanelVariant;
  logs: DiagnosticRecordDto[];
  filteredLogs: DiagnosticRecordDto[];
  total: number;
  loading: boolean;
  subscriptionStatus: ReturnType<typeof useDiagnosticSubscription>['status'];
  filter: ReturnType<typeof useLogStore.getState>['filter'];
  activeLogDomainTab: ReturnType<typeof useLogStore.getState>['activeLogDomainTab'];
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

function sameDiagnostic(left: DiagnosticRecordDto, right: DiagnosticRecordDto): boolean {
  return left.streamId === right.streamId && left.sequence === right.sequence;
}

export function useLogPanelController(variant: LogPanelVariant): LogPanelController {
  const { entries: logs, streamId } = useLiveLogs();
  const { status: subscriptionStatus, reconnect } = useDiagnosticSubscription();
  const filter = useLogStore((state) => state.filter);
  const activeLogDomainTab = useLogStore((state) => state.activeLogDomainTab);
  const toggleLevel = useLogStore((state) => state.toggleLevel);
  const setSearchText = useLogStore((state) => state.setSearchText);
  const selectedLog = useLogStore((state) => state.selectedLog);
  const setSelectedLog = useLogStore((state) => state.setSelectedLog);
  const [autoScroll, setAutoScroll] = useState(true);
  const [refreshScrollToken, setRefreshScrollToken] = useState(0);
  const { dragHandleRef, dragHandleProps } = useLogPanelDetach(variant);

  const filteredLogs = useMemo(() => applyLogFilter(logs, filter), [logs, filter]);
  const selectedIndex = selectedLog
    ? filteredLogs.findIndex((log) => sameDiagnostic(log, selectedLog))
    : null;
  const loading = subscriptionStatus === 'connecting';

  const handleClose = useCallback(() => {
    if (variant === 'embedded') togglePanelCollapsed();
    else void getCurrentWindow().close();
  }, [variant]);

  const handleSelectLog = useCallback((index: number) => {
    const log = filteredLogs[index] ?? null;
    setSelectedLog(log);
    if (log) useEditorStore.getState().setDetailFocus({ kind: 'log' });
    else useEditorStore.getState().clearDetailFocus();
  }, [filteredLogs, setSelectedLog]);

  const clearLogs = useCallback(() => {
    logBuffer.clear();
    setSelectedLog(null);
  }, [setSelectedLog]);

  const refreshLogs = useCallback(() => {
    reconnect();
    if (autoScroll) setRefreshScrollToken((token) => token + 1);
  }, [autoScroll, reconnect]);

  return {
    variant,
    logs,
    filteredLogs,
    total: logs.length,
    loading,
    subscriptionStatus,
    filter,
    activeLogDomainTab,
    autoScroll,
    setAutoScroll,
    refreshScrollToken,
    isInitialLoad: loading && streamId === null && logs.length === 0,
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
