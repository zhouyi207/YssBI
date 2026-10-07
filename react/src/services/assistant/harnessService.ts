import type { LanguageModelSelection } from "./modelContract";
import { citationDetailSchema, type CitationDetail } from "./knowledgeService";
import { Channel } from "@tauri-apps/api/core";

import { trackChannel, untrackChannel } from "@/services/devHmrIpc";
import { invokeCommand } from "@/services/ipc";
import { clearChannelMessageHandler } from "@/shared/platform/tauriWebview";

import {
  parseHarnessEvent,
  parseHarnessSession,
  parseHarnessSubscription,
  parseHarnessTurnResult,
  parseHarnessToolInspection,
  DEFAULT_TURN_OPTIONS,
  type HarnessTurnOptions,
  type HarnessToolInspection,
  type HarnessKnowledgeCitation,
  type HarnessEvent,
  type HarnessSession,
  type HarnessTurnResult,
} from "./harnessContract";

export type {
  HarnessEvent,
  HarnessKnowledgeCitation,
  HarnessSession,
  HarnessTurnResult,
} from "./harnessContract";

export interface HarnessEventSubscription {
  unsubscribe(): Promise<void>;
}

export class HarnessService {
  static async inspectTool(
    sessionId: string,
    invocationId: string,
  ): Promise<HarnessToolInspection> {
    return parseHarnessToolInspection(
      await invokeCommand("inspect_harness_tool", { sessionId, invocationId }),
    );
  }

  static async inspectCitation(
    sessionId: string,
    citation: HarnessKnowledgeCitation,
  ): Promise<CitationDetail> {
    return citationDetailSchema.parse(
      await invokeCommand("inspect_harness_citation", { sessionId, citation }),
    );
  }
  static async selectModel(
    sessionId: string,
    model: LanguageModelSelection,
  ): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("select_harness_model", { sessionId, model }));
  }

  static async createSession(): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("create_harness_session"));
  }

  static async openSession(sessionId: string): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("open_harness_session", { sessionId }));
  }

  static async renameSession(sessionId: string, title: string): Promise<HarnessSession> {
    return parseHarnessSession(await invokeCommand("rename_harness_session", { sessionId, title }));
  }

  static async subscribeEvents(
    sessionId: string,
    afterSequence: number,
    onEvent: (event: HarnessEvent) => void,
    onError: (error: unknown) => void,
  ): Promise<HarnessEventSubscription> {
    let subscriptionId: string | null = null;
    let cleaned = false;
    let unsubscribed = false;
    const unsubscribeRemote = async () => {
      if (unsubscribed || !subscriptionId) return;
      unsubscribed = true;
      await HarnessService.unsubscribeEvents(subscriptionId);
    };
    const channel = trackChannel(new Channel<unknown>(), () => {
      cleanup();
      void unsubscribeRemote().catch(() => {});
    });
    const cleanup = () => {
      if (cleaned) return;
      cleaned = true;
      untrackChannel(channel);
      clearChannelMessageHandler(channel);
    };
    channel.onmessage = (value) => {
      if (cleaned) return;
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
      if (cleaned) {
        await unsubscribeRemote().catch(() => {});
        throw new Error("Harness subscription was disposed before activation");
      }
    } catch (error) {
      cleanup();
      throw error;
    }
    return {
      unsubscribe: async () => {
        cleanup();
        await unsubscribeRemote();
      },
    };
  }

  static async submitTurn(
    sessionId: string,
    message: string,
    resources: readonly import("@/shared/types/domain/resource").ResourceRef[] = [],
    model: LanguageModelSelection | null = null,
    options: HarnessTurnOptions = DEFAULT_TURN_OPTIONS,
  ): Promise<HarnessTurnResult> {
    return parseHarnessTurnResult(
      await invokeCommand("submit_harness_turn", { sessionId, message, resources, model, options }),
    );
  }

  static async cancelTurn(sessionId: string): Promise<void> {
    await invokeCommand("cancel_harness_turn", { sessionId });
  }

  private static async unsubscribeEvents(subscriptionId: string): Promise<void> {
    await invokeCommand("unsubscribe_harness_events", { subscriptionId });
  }
}
