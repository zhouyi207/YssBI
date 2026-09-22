import { Channel } from "@tauri-apps/api/core";

import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { invokeCommand } from "@/services/ipc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";

import {
  parseHarnessEvent,
  parseHarnessMemoryRecords,
  parseHarnessRuntimeStatus,
  parseHarnessSession,
  parseHarnessSessions,
  parseHarnessSubscription,
  parseHarnessTurnResult,
  type HarnessEvent,
  type HarnessMemoryRecord,
  type HarnessRuntimeStatus,
  type HarnessSession,
  type HarnessTurnResult,
} from "./harnessContract";

export type {
  HarnessEvent,
  HarnessKnowledgeCitation,
  HarnessMemoryRecord,
  HarnessRuntimeStatus,
  HarnessSession,
  HarnessTurnResult,
} from "./harnessContract";

export interface HarnessEventSubscription {
  unsubscribe(): Promise<void>;
}

export class HarnessService {
  static async runtimeStatus(): Promise<HarnessRuntimeStatus> {
    return parseHarnessRuntimeStatus(await invokeCommand("get_harness_runtime_status"));
  }

  static async configureProvider(
    model: string,
    baseUrl: string,
    apiKey: string,
  ): Promise<HarnessRuntimeStatus> {
    return parseHarnessRuntimeStatus(
      await invokeCommand("configure_harness_provider", {
        request: { model, baseUrl, apiKey },
      }),
    );
  }

  static async createSession(): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("create_harness_session"));
  }

  static async listSessions(): Promise<readonly HarnessSession[]> {
    return parseHarnessSessions(await invokeCommand("list_harness_sessions"));
  }

  static async openSession(sessionId: string): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("open_harness_session", { sessionId }));
  }

  static async subscribeEvents(
    sessionId: string,
    afterSequence: number,
    onEvent: (event: HarnessEvent) => void,
    onError: (error: unknown) => void,
  ): Promise<HarnessEventSubscription> {
    let subscriptionId: string | null = null;
    let cleaned = false;
    const channel = trackChannel(new Channel<unknown>(), () => {
      cleanup();
      if (subscriptionId) void HarnessService.unsubscribeEvents(subscriptionId).catch(() => {});
    });
    const cleanup = () => {
      if (cleaned) return;
      cleaned = true;
      untrackChannel(channel);
      clearChannelMessageHandler(channel);
    };
    channel.onmessage = (value) => {
      try {
        onEvent(parseHarnessEvent(value));
      } catch (error) {
        onError(error);
      }
    };
    try {
      const snapshot = parseHarnessSubscription(
        await invokeCommand("subscribe_harness_events", {
          sessionId,
          afterSequence,
          onEvent: channel,
        }),
      );
      subscriptionId = snapshot.subscriptionId;
      if (cleaned) await HarnessService.unsubscribeEvents(subscriptionId).catch(() => {});
    } catch (error) {
      cleanup();
      throw error;
    }
    let unsubscribed = false;
    return {
      unsubscribe: async () => {
        if (unsubscribed) return;
        unsubscribed = true;
        cleanup();
        if (subscriptionId) await HarnessService.unsubscribeEvents(subscriptionId);
      },
    };
  }

  static async submitTurn(
    sessionId: string,
    message: string,
    activeGraphPath: string | null = null,
  ): Promise<HarnessTurnResult> {
    return parseHarnessTurnResult(
      await invokeCommand("submit_harness_turn", { sessionId, message, activeGraphPath }),
    );
  }

  static async cancelTurn(sessionId: string): Promise<void> {
    await invokeCommand("cancel_harness_turn", { sessionId });
  }

  static async listMemory(sessionId: string): Promise<readonly HarnessMemoryRecord[]> {
    return parseHarnessMemoryRecords(await invokeCommand("list_harness_memory", { sessionId }));
  }

  static async deleteMemory(sessionId: string, recordId: string): Promise<void> {
    await invokeCommand("delete_harness_memory", { sessionId, recordId });
  }

  static async unsubscribeEvents(subscriptionId: string): Promise<void> {
    await invokeCommand("unsubscribe_harness_events", { subscriptionId });
  }
}
