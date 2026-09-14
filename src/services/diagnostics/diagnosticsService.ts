import { invokeCommand } from "@/services/ipc";
import { subscribeRecords, type RecordSubscription } from "@/services/ipc/recordSubscription";
import type {
  DiagnosticBatchDto,
  DiagnosticSubscriptionDto,
  FrontendDiagnosticEntryDto,
} from "@/shared/types/dto/diagnostics";
import {
  parseDiagnosticBatchDto,
  parseDiagnosticSubscriptionDto,
} from "@/shared/types/dto/diagnosticsParser";

export type DiagnosticSubscription = RecordSubscription<DiagnosticSubscriptionDto>;

/** Application-owned runtime diagnostics; independent of persisted plugin logs. */
export class DiagnosticsService {
  static async submitFrontendDiagnostics(
    entries: readonly FrontendDiagnosticEntryDto[],
  ): Promise<void> {
    if (entries.length === 0) return;
    await invokeCommand("submit_frontend_diagnostics", { entries: [...entries] });
  }

  static subscribeDiagnostics(
    onRecords: (batch: DiagnosticBatchDto) => void,
    onDiscontinuity?: (error: unknown) => void,
  ): Promise<DiagnosticSubscription> {
    return subscribeRecords(
      {
        subscribeCommand: "subscribe_diagnostics",
        unsubscribeCommand: "unsubscribe_diagnostics",
        parseSnapshot: parseDiagnosticSubscriptionDto,
        parseBatch: parseDiagnosticBatchDto,
      },
      onRecords,
      onDiscontinuity,
    );
  }

  static async unsubscribeDiagnostics(subscriptionId: string): Promise<void> {
    await invokeCommand("unsubscribe_diagnostics", { subscriptionId });
  }
}
