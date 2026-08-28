import { useCallback, useMemo, useState } from 'react';
import { revealDetails } from '@/features/application/editor';
import {
  useDiagnosticSubscription,
  type DiagnosticSubscriptionStatus,
} from '@/features/application/log';
import { useEditorStore, logBuffer, useLiveLogs } from '@/features/application/viewCapabilities';
import {
  useLogStore,
  type DiagnosticLogFilter,
} from '@/features/application/viewCapabilities';
import type {
  DiagnosticLevel,
  DiagnosticRecordDto,
} from '@/shared/types/dto/diagnostics';

export interface LogWorkspaceController {
  readonly logs: readonly DiagnosticRecordDto[];
  readonly filter: DiagnosticLogFilter;
  readonly selectedLog: DiagnosticRecordDto | null;
  readonly autoScroll: boolean;
  readonly loading: boolean;
  readonly isInitialLoad: boolean;
  readonly subscriptionStatus: DiagnosticSubscriptionStatus;
  readonly refreshScrollToken: number;
  readonly toggleLevel: (level: DiagnosticLevel) => void;
  readonly setSearchText: (text: string) => void;
  readonly setAutoScroll: (autoScroll: boolean) => void;
  readonly refreshLogs: () => void;
  readonly clearLogs: () => void;
  readonly selectLog: (log: DiagnosticRecordDto | null) => void;
}

export function useLogWorkspaceController(): LogWorkspaceController {
  const { entries: logs, streamId } = useLiveLogs();
  const { status: subscriptionStatus, reconnect } = useDiagnosticSubscription();
  const filter = useLogStore((state) => state.filter);
  const selectedLog = useLogStore((state) => state.selectedLog);
  const autoScroll = useLogStore((state) => state.autoScroll);
  const toggleLevel = useLogStore((state) => state.toggleLevel);
  const setSearchText = useLogStore((state) => state.setSearchText);
  const setAutoScroll = useLogStore((state) => state.setAutoScroll);
  const setSelectedLog = useLogStore((state) => state.setSelectedLog);
  const [refreshScrollToken, setRefreshScrollToken] = useState(0);

  const selectLog = useCallback((log: DiagnosticRecordDto | null) => {
    setSelectedLog(log);
    if (log) {
      void revealDetails({ kind: 'log' });
      return;
    }

    const editorStore = useEditorStore.getState();
    if (editorStore.detailFocus?.kind === 'log') {
      editorStore.clearDetailFocus();
    }
  }, [setSelectedLog]);

  const clearLogs = useCallback(() => {
    logBuffer.clear();
    selectLog(null);
  }, [selectLog]);

  const refreshLogs = useCallback(() => {
    reconnect();
    if (useLogStore.getState().autoScroll) {
      setRefreshScrollToken((token) => token + 1);
    }
  }, [reconnect]);

  const loading = subscriptionStatus === 'connecting';
  const isInitialLoad = loading && streamId === null && logs.length === 0;

  return useMemo(() => ({
    logs,
    filter,
    selectedLog,
    autoScroll,
    loading,
    isInitialLoad,
    subscriptionStatus,
    refreshScrollToken,
    toggleLevel,
    setSearchText,
    setAutoScroll,
    refreshLogs,
    clearLogs,
    selectLog,
  }), [
    autoScroll,
    clearLogs,
    filter,
    isInitialLoad,
    loading,
    logs,
    refreshLogs,
    refreshScrollToken,
    selectLog,
    selectedLog,
    setAutoScroll,
    setSearchText,
    subscriptionStatus,
    toggleLevel,
  ]);
}
