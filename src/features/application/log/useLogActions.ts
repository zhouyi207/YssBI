import { useCallback } from "react";
import { logBuffer } from "@/features/core/log/logBuffer";
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
    logBuffer.setLoading(true);
    try {
      const response = await LogService.getLogs(offset, limit);
      const fileLogs = LogService.normalizeLogResponse(response);
      const total = await getLogCountFallback(fileLogs.length);
      logBuffer.setInitial(fileLogs, total, offset + fileLogs.length < total);
    } catch (error) {
      console.error("[LogActions] Failed to load logs:", error);
      logBuffer.setLoading(false);
    }
  }, []);

  const loadMoreLogs = useCallback(async () => {
    // offset 用「已从后端加载的历史条数」，不受实时追加影响（修复 logs.length 偏移 bug）
    const offset = logBuffer.getBackendCount();
    logBuffer.setLoading(true);
    try {
      const response = await LogService.getLogs(offset, DEFAULT_MORE_LIMIT);
      const olderLogs = LogService.normalizeLogResponse(response);
      const total = await getLogCountFallback(offset + olderLogs.length);
      logBuffer.prependOlder(olderLogs, total, offset + olderLogs.length < total);
    } catch (error) {
      console.error("[LogActions] Failed to load more logs:", error);
      logBuffer.setLoading(false);
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
