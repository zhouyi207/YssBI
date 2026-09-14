import { useCallback, useMemo, useState } from "react";
import { revealDetails } from "@/features/application/editor";
import { useLogSubscription, type LogSubscriptionStatus } from "@/features/application/log";
import { useEditorUi } from "@/features/core/editor/ui";
import { editorUi } from "@/features/core/editor/ui";
import { logBuffer, useLiveLogs } from "@/features/application/log";
import { useLogStore } from "@/features/application/log";
import type { LogLogFilter } from "@/features/application/log";
import type { LogLevel, LogRecordDto } from "@/shared/types/domain/log";

export interface LogWorkspaceController {
  readonly logs: readonly LogRecordDto[];
  readonly filter: LogLogFilter;
  readonly selectedLog: LogRecordDto | null;
  readonly autoScroll: boolean;
  readonly loading: boolean;
  readonly isInitialLoad: boolean;
  readonly subscriptionStatus: LogSubscriptionStatus;
  readonly continuity: "complete" | "truncated" | "disconnected";
  readonly refreshScrollToken: number;
  readonly toggleLevel: (level: LogLevel) => void;
  readonly setSearchText: (text: string) => void;
  readonly setAutoScroll: (autoScroll: boolean) => void;
  readonly refreshLogs: () => void;
  readonly clearLogs: () => void;
  readonly selectLog: (log: LogRecordDto | null) => void;
}

export function useLogWorkspaceController(): LogWorkspaceController {
  const { entries: logs, streamId, truncated } = useLiveLogs();
  const { status: subscriptionStatus, reconnect } = useLogSubscription();
  const filter = useLogStore((state) => state.filter);
  const selectedLog = useLogStore((state) => state.selectedLog);
  const autoScroll = useLogStore((state) => state.autoScroll);
  const toggleLevel = useLogStore((state) => state.toggleLevel);
  const setSearchText = useLogStore((state) => state.setSearchText);
  const setAutoScroll = useLogStore((state) => state.setAutoScroll);
  const setSelectedLog = useLogStore((state) => state.setSelectedLog);
  const detailFocus = useEditorUi((state) => state.detailFocus);
  const clearDetailFocus = editorUi.clearDetailFocus;
  const [refreshScrollToken, setRefreshScrollToken] = useState(0);

  const selectLog = useCallback(
    (log: LogRecordDto | null) => {
      setSelectedLog(log);
      if (log) {
        void revealDetails({ kind: "log" });
        return;
      }

      if (detailFocus?.kind === "log") {
        clearDetailFocus();
      }
    },
    [clearDetailFocus, detailFocus, setSelectedLog],
  );

  const clearLogs = useCallback(() => {
    logBuffer.clear();
    selectLog(null);
  }, [selectLog]);

  const refreshLogs = useCallback(() => {
    reconnect();
    if (autoScroll) {
      setRefreshScrollToken((token) => token + 1);
    }
  }, [autoScroll, reconnect]);

  const loading = subscriptionStatus === "connecting";
  const continuity =
    subscriptionStatus === "error"
      ? "disconnected"
      : truncated
        ? "truncated"
        : subscriptionStatus === "live"
          ? "complete"
          : "disconnected";
  const isInitialLoad = loading && streamId === null && logs.length === 0;

  return useMemo(
    () => ({
      logs,
      filter,
      selectedLog,
      autoScroll,
      loading,
      isInitialLoad,
      subscriptionStatus,
      continuity,
      refreshScrollToken,
      toggleLevel,
      setSearchText,
      setAutoScroll,
      refreshLogs,
      clearLogs,
      selectLog,
    }),
    [
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
      continuity,
      toggleLevel,
    ],
  );
}
