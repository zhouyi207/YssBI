import { invokeCommand } from "@/services/ipc";
import { subscribeRecords, type RecordSubscription } from "@/services/ipc/recordSubscription";
import type { FrontendLogEntryDto, LogBatchDto, LogSubscriptionDto } from "@/shared/types/dto/log";
import { parseLogBatchDto, parseLogSubscriptionDto } from "@/shared/types/dto/logParser";

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
}
