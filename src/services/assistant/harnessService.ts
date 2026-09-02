import { Channel } from "@tauri-apps/api/core";

import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { invokeCommand } from "@/services/ipc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";

import {
  parseHarnessEvent,
  parseHarnessMemoryRecords,
  parseHarnessRuntimeStatus,
  parseHarnessSession,
  parseHarnessSubscription,
  parseHarnessTurnResult,
  parseHarnessWorkflowRun,
  type HarnessEvent,
  type HarnessMemoryRecord,
  type HarnessRuntimeStatus,
  type HarnessSession,
  type HarnessTurnResult,
  type HarnessWorkflowRun,
} from "./harnessContract";

export type {
  HarnessEvent,
  HarnessKnowledgeCitation,
  HarnessMemoryRecord,
  HarnessRuntimeStatus,
  HarnessSession,
  HarnessTurnResult,
  HarnessWorkflowRun,
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

  static async submitTurn(sessionId: string, message: string): Promise<HarnessTurnResult> {
    return parseHarnessTurnResult(
      await invokeCommand("submit_harness_turn", { sessionId, message }),
    );
  }

  static async cancelTurn(sessionId: string): Promise<void> {
    await invokeCommand("cancel_harness_turn", { sessionId });
  }

  static async closeSession(sessionId: string): Promise<void> {
    await invokeCommand("close_harness_session", { sessionId });
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

  static async planDatasetQualityReview(
    sessionId: string,
    turnId: string,
    databaseId: string,
  ): Promise<HarnessWorkflowRun> {
    return parseHarnessWorkflowRun(
      await invokeCommand("plan_dataset_quality_review", { sessionId, turnId, databaseId }),
    );
  }

  static async advanceWorkflow(runId: string): Promise<HarnessWorkflowRun> {
    return parseHarnessWorkflowRun(await invokeCommand("advance_harness_workflow", { runId }));
  }

  static async pauseWorkflow(runId: string): Promise<HarnessWorkflowRun> {
    return parseHarnessWorkflowRun(await invokeCommand("pause_harness_workflow", { runId }));
  }

  static async resumeWorkflow(runId: string): Promise<HarnessWorkflowRun> {
    return parseHarnessWorkflowRun(await invokeCommand("resume_harness_workflow", { runId }));
  }

  static async cancelWorkflow(runId: string): Promise<HarnessWorkflowRun> {
    return parseHarnessWorkflowRun(await invokeCommand("cancel_harness_workflow", { runId }));
  }
}
