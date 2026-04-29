import { useCallback } from "react";
import { useLogStore } from "@/features/core/log/logStore";
import { LogService } from "@/services/log";

const DEFAULT_INITIAL_LIMIT = 100;
const DEFAULT_MORE_LIMIT = 50;
const DEFAULT_REFRESH_LIMIT = 200;

async function getLogCountFallback(fallback: number) {
  try {
    const count = await LogService.getLogCount();
    return typeof count === "number" ? count : fallback;
  } catch {
    return fallback;
  }
}

export function useLogActions() {
  const loadLogs = useCallback(async (offset = 0, limit = DEFAULT_INITIAL_LIMIT) => {
    const store = useLogStore.getState();
    store.setLogPageState({ loading: true });

    try {
      const response = await LogService.getLogs(offset, limit);
      const fileLogs = LogService.normalizeLogResponse(response);
      const total = await getLogCountFallback(fileLogs.length);
      store.setLogs(fileLogs);
      store.setLogPageState({
        total,
        hasMore: offset + fileLogs.length < total,
        loading: false,
      });
    } catch (error) {
      console.error("[LogActions] Failed to load logs:", error);
      store.setLogPageState({ loading: false });
    }
  }, []);

  const loadMoreLogs = useCallback(async () => {
    const store = useLogStore.getState();
    const { logs, hasMore, loading } = store;
    if (!hasMore || loading) return;

    store.setLogPageState({ loading: true });
    try {
      const offset = Array.isArray(logs) ? logs.length : 0;
      const response = await LogService.getLogs(offset, DEFAULT_MORE_LIMIT);
      const olderLogs = LogService.normalizeLogResponse(response);
      const total = await getLogCountFallback(offset + olderLogs.length);
      store.prependLogs(olderLogs);
      store.setLogPageState({
        total,
        hasMore: offset + olderLogs.length < total,
        loading: false,
      });
    } catch (error) {
      console.error("[LogActions] Failed to load more logs:", error);
      store.setLogPageState({ loading: false });
    }
  }, []);

  const refreshLogs = useCallback(async () => {
    const limit = await getLogCountFallback(DEFAULT_REFRESH_LIMIT);
    await loadLogs(0, limit);
  }, [loadLogs]);

  return {
    loadLogs,
    loadMoreLogs,
    refreshLogs,
  };
}
