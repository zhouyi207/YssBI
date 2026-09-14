import { invokeCommand } from "@/services/ipc";
import { subscribeRecords, type RecordSubscription } from "@/services/ipc/recordSubscription";
import type {
  FrontendLogEntryDto,
  LogBatchDto,
  LogPage,
  LogQuery,
  LogStatistics,
  LogSubscriptionDto,
} from "@/shared/types/dto/log";
import {
  parseLogBatchDto,
  parseLogPage,
  parseLogStatistics,
  parseLogSubscriptionDto,
} from "@/shared/types/dto/logParser";

export type LogSubscription = RecordSubscription<LogSubscriptionDto>;
export type FrontendLogEntry = FrontendLogEntryDto;

/** Structured runtime observations use the plugin log registry and persisted history. */
export class LogService {
  static async submitFrontendLogs(entries: readonly FrontendLogEntryDto[]): Promise<void> {
    if (entries.length === 0) return;
    await invokeCommand("plugin:tracing|submit_frontend_logs", { entries: [...entries] });
  }

  static subscribeLogs(
    onRecords: (batch: LogBatchDto) => void,
    onDiscontinuity?: (error: unknown) => void,
  ): Promise<LogSubscription> {
    return subscribeRecords(
      {
        subscribeCommand: "plugin:tracing|subscribe_logs",
        unsubscribeCommand: "plugin:tracing|unsubscribe_logs",
        parseSnapshot: parseLogSubscriptionDto,
        parseBatch: parseLogBatchDto,
      },
      onRecords,
      onDiscontinuity,
    );
  }

  static async unsubscribeLogs(subscriptionId: string): Promise<void> {
    await invokeCommand("plugin:tracing|unsubscribe_logs", { subscriptionId });
  }

  static async queryLogs(query: LogQuery = {}): Promise<LogPage> {
    return parseLogPage(await invokeCommand("plugin:tracing|query_logs", { query }));
  }

  static async logStatistics(): Promise<LogStatistics> {
    return parseLogStatistics(await invokeCommand("plugin:tracing|log_statistics"));
  }
}
