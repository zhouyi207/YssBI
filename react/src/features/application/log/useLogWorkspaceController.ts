import { useCallback, useMemo, useState } from "react";
import { revealDetails } from "@/features/application/editor";
import { editorUi } from "@/features/core/editor/ui";
import type { LogRecordDto } from "@/shared/types/domain/log";
import { useLogSubscription, type LogSubscriptionStatus } from "./useLogSubscription";
import { logBuffer } from "./logBuffer";
import { useLogStore } from "./logStore";

export interface LogWorkspaceController {
  readonly subscriptionStatus: LogSubscriptionStatus;
  readonly refreshScrollToken: number;
  readonly refreshLogs: () => void;
  readonly clearLogs: () => void;
  readonly selectLog: (log: LogRecordDto | null) => void;
}

export function useLogWorkspaceController(): LogWorkspaceController {
  const { status: subscriptionStatus, reconnect } = useLogSubscription();
  const [refreshScrollToken, setRefreshScrollToken] = useState(0);

  const selectLog = useCallback((log: LogRecordDto | null) => {
    useLogStore.getState().setSelectedLog(log);
    if (log) {
      void revealDetails({ kind: "log" });
    } else if (editorUi.getSnapshot().detailFocus?.kind === "log") {
      editorUi.clearDetailFocus();
    }
  }, []);

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

  return useMemo(
    () => ({
      subscriptionStatus,
      refreshScrollToken,
      refreshLogs,
      clearLogs,
      selectLog,
    }),
    [clearLogs, refreshLogs, refreshScrollToken, selectLog, subscriptionStatus],
  );
}
