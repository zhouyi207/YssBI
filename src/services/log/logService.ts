import { Channel } from "@tauri-apps/api/core";
import { invokeCommand } from "@/services/ipc";
import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";
import type { DiagnosticBatchDto, DiagnosticSubscriptionDto } from "@/shared/types/dto/diagnostics";
import { parseDiagnosticSubscriptionDto } from "@/shared/types/dto/diagnosticsParser";
import {
  createDiagnosticBatchReceiver,
  DiagnosticStreamDiscontinuityError,
} from "./diagnosticBatchReceiver";

export interface FrontendDiagnosticEntry {
  readonly level: "trace" | "debug" | "info" | "warn" | "error";
  readonly domain: "application" | "execution" | "system" | "graph" | "data" | "ui";
  readonly target: string;
  readonly event?: string;
  readonly message: string;
  readonly source?: string;
  readonly fields: Record<string, unknown>;
}

const MAX_SUBSCRIPTION_ATTEMPTS = 2;

export interface DiagnosticSubscription {
  snapshot: DiagnosticSubscriptionDto;
  /** Delivers Channel batches queued while the initial snapshot was in flight. */
  activate: () => void;
  unsubscribe: () => Promise<void>;
}

export class LogService {
  static async submitFrontendDiagnostics(
    entries: readonly FrontendDiagnosticEntry[],
  ): Promise<void> {
    if (entries.length === 0) return;
    await invokeCommand("submit_frontend_diagnostics", { entries: [...entries] });
  }

  static async subscribeDiagnostics(
    onRecords: (batch: DiagnosticBatchDto) => void,
  ): Promise<DiagnosticSubscription> {
    for (let attempt = 0; attempt < MAX_SUBSCRIPTION_ATTEMPTS; attempt += 1) {
      const receiver = createDiagnosticBatchReceiver(onRecords);
      let subscriptionId: string | null = null;
      let hmrDisposed = false;
      let cleaned = false;
      let channel: Channel<unknown> | null = null;
      const cleanupChannel = () => {
        if (cleaned) return;
        cleaned = true;
        receiver.dispose();
        if (channel) {
          untrackChannel(channel);
          clearChannelMessageHandler(channel);
        }
      };
      channel = trackChannel(new Channel<unknown>(), () => {
        hmrDisposed = true;
        cleanupChannel();
        if (subscriptionId) {
          void LogService.unsubscribeDiagnostics(subscriptionId).catch(() => {});
        }
      });
      channel.onmessage = receiver.onmessage;

      let snapshot: DiagnosticSubscriptionDto;
      try {
        snapshot = parseDiagnosticSubscriptionDto(
          await invokeCommand("subscribe_diagnostics", { onRecords: channel }),
        );
        subscriptionId = snapshot.subscriptionId;
      } catch (error) {
        cleanupChannel();
        throw error;
      }

      if (hmrDisposed || receiver.isDisposed()) {
        await LogService.unsubscribeDiagnostics(snapshot.subscriptionId).catch(() => {});
        throw new Error("Diagnostic subscription was disposed before activation");
      }

      const discontinuity = receiver.prepare(snapshot);
      if (discontinuity) {
        cleanupChannel();
        await LogService.unsubscribeDiagnostics(snapshot.subscriptionId).catch(() => {});
        if (attempt + 1 < MAX_SUBSCRIPTION_ATTEMPTS) continue;
        throw new DiagnosticStreamDiscontinuityError(discontinuity);
      }

      let unsubscribed = false;
      const unsubscribe = async () => {
        if (unsubscribed) return;
        unsubscribed = true;
        cleanupChannel();
        await LogService.unsubscribeDiagnostics(snapshot.subscriptionId);
      };
      return {
        snapshot,
        activate: () => {
          try {
            receiver.activate();
          } catch (error) {
            cleanupChannel();
            if (!unsubscribed) {
              unsubscribed = true;
              void LogService.unsubscribeDiagnostics(snapshot.subscriptionId).catch(() => {});
            }
            throw error;
          }
        },
        unsubscribe,
      };
    }

    throw new Error("Diagnostic subscription attempts exhausted");
  }

  static async unsubscribeDiagnostics(subscriptionId: string): Promise<void> {
    await invokeCommand("unsubscribe_diagnostics", { subscriptionId });
  }
}
