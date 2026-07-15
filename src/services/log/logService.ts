import { invoke } from "@tauri-apps/api/core";
import { LogLevel, LogMessage, LogType } from "@/shared/types/ui";

export class LogService {
  static async getLogs(offset: number, limit: number): Promise<LogMessage[]> {
    return invoke<LogMessage[]>("get_logs", { offset, limit });
  }

  static async getLogCount(): Promise<number> {
    return invoke<number>("get_log_count");
  }

  static async frontendLog(level: LogLevel, logType: LogType, message: string, source?: string): Promise<void> {
    await invoke("frontend_log", {
      level,
      logType,
      message,
      source: source ?? null,
    });
  }

  static normalizeLogResponse(response: unknown): LogMessage[] {
    return Array.isArray(response) ? response : (response as { logs?: LogMessage[] } | null)?.logs ?? [];
  }
}
