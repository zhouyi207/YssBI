import { createStore } from "zustand/vanilla";
import { toErrorReference } from "@/features/application/errorReference";
import { getActiveGraphContext } from "@/features/application/editor/editorGroupContext";
import {
  HarnessService,
  type HarnessEvent,
  type HarnessSession,
  type HarnessEventSubscription,
} from "@/services/assistant/harnessService";
import {
  INITIAL_SNAPSHOT,
  reduceHarnessEvent,
  type AssistantHarnessSnapshot,
} from "./assistantHarnessProjection";

const TERMINAL_TURN_ERRORS = new Set([
  "assistant_provider_unavailable",
  "assistant_authentication_failed",
  "assistant_rate_limited",
  "assistant_provider_request_rejected",
  "assistant_context_window_exceeded",
  "assistant_provider_output_truncated",
  "assistant_provider_stream_interrupted",
  "assistant_provider_content_filtered",
  "assistant_provider_payment_required",
  "assistant_provider_connection_failed",
  "assistant_invalid_provider_response",
  "assistant_turn_failed",
  "assistant_turn_timed_out",
  "harness_turn_cancelled",
  "invalid_harness_request",
]);

export class AssistantHarnessProjection {
  private readonly store = createStore<AssistantHarnessSnapshot>(() => INITIAL_SNAPSHOT);
  readonly getState = this.store.getState;
  readonly getInitialState = this.store.getInitialState;
  readonly subscribe = this.store.subscribe;

  private get snapshot(): AssistantHarnessSnapshot {
    return this.store.getState();
  }
  private subscription: HarnessEventSubscription | null = null;
  private generation = 0;
  private streamGeneration = 0;
  private providerRequest = 0;
  private recovering = false;
  private submitting = false;
  readonly start = async (): Promise<void> => {
    this.stop();
    const generation = ++this.generation;
    this.update({ ...INITIAL_SNAPSHOT, status: "initializing" });
    try {
      const conversations = await HarnessService.listSessions();
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        conversations,
      });
      const session =
        conversations.length > 0
          ? await HarnessService.openSession(conversations[0].sessionId)
          : await HarnessService.createSession();
      if (generation !== this.generation) return;
      await this.attachSession(session, generation);
    } catch (error) {
      if (generation === this.generation)
        this.update({
          ...this.snapshot,
          status: "error",
          error: toErrorReference(error, "assistant_session_failed"),
        });
    }
  };

  readonly newConversation = async (): Promise<void> => {
    if (this.snapshot.isRunning || this.submitting || this.snapshot.status === "initializing")
      return;
    if (this.snapshot.sessionId && this.snapshot.messages.length === 0) return;
    await this.changeConversation(null);
  };

  readonly selectConversation = async (sessionId: string): Promise<void> => {
    if (
      sessionId === this.snapshot.sessionId ||
      this.snapshot.isRunning ||
      this.submitting ||
      this.snapshot.status === "initializing"
    )
      return;
    await this.changeConversation(sessionId);
  };

  private async changeConversation(sessionId: string | null): Promise<void> {
    const conversations = this.snapshot.conversations;
    const providerConfigured = this.snapshot.providerConfigured;
    this.stop();
    const generation = ++this.generation;
    this.update({ ...INITIAL_SNAPSHOT, conversations, providerConfigured });
    try {
      const session = sessionId
        ? await HarnessService.openSession(sessionId)
        : await HarnessService.createSession();
      if (generation !== this.generation) return;
      await this.attachSession(session, generation);
    } catch (error) {
      if (generation === this.generation)
        this.update({
          ...this.snapshot,
          status: "error",
          error: toErrorReference(error, "assistant_session_failed"),
        });
    }
  }

  private async attachSession(session: HarnessSession, generation: number): Promise<void> {
    this.update({
      ...this.snapshot,
      sessionId: session.sessionId,
      conversations: [
        session,
        ...this.snapshot.conversations.filter((entry) => entry.sessionId !== session.sessionId),
      ],
    });
    const memoryRecords = await HarnessService.listMemory(session.sessionId);
    if (generation !== this.generation) return;
    // Replay follows the initial memory snapshot so late snapshot data cannot undo events.
    this.update({ ...this.snapshot, memoryRecords, memoryCount: memoryRecords.length });
    const streamGeneration = ++this.streamGeneration;
    const subscription = await HarnessService.subscribeEvents(
      session.sessionId,
      0,
      (event) => {
        if (generation === this.generation && streamGeneration === this.streamGeneration)
          this.onEvent(event);
      },
      () => {
        if (generation === this.generation && streamGeneration === this.streamGeneration)
          this.onStreamError();
      },
    );
    if (generation !== this.generation || streamGeneration !== this.streamGeneration) {
      await subscription.unsubscribe();
      return;
    }
    this.subscription = subscription;
    this.update({
      ...this.snapshot,
      status: this.snapshot.providerConfigured ? "ready" : "provider-unavailable",
    });
  }

  private async refreshConversations(generation: number): Promise<void> {
    try {
      const conversations = await HarnessService.listSessions();
      if (generation === this.generation) this.update({ ...this.snapshot, conversations });
    } catch (error) {
      if (generation === this.generation)
        this.update({
          ...this.snapshot,
          error: toErrorReference(error, "assistant_session_failed"),
        });
    }
  }

  readonly invalidateProvider = (): void => {
    this.providerRequest += 1;
    this.update({ ...this.snapshot, providerConfigured: false });
  };

  readonly syncProvider = async (model: string, baseUrl: string, apiKey: string): Promise<void> => {
    const generation = this.generation;
    const request = ++this.providerRequest;
    try {
      const runtime = await HarnessService.configureProvider(model, baseUrl, apiKey);
      if (generation !== this.generation || request !== this.providerRequest) return;
      this.update({
        ...this.snapshot,
        providerConfigured: runtime.providerConfigured,
        error: null,
        status:
          runtime.providerConfigured &&
          this.snapshot.sessionId &&
          this.subscription &&
          (this.snapshot.status === "ready" || this.snapshot.status === "provider-unavailable")
            ? "ready"
            : runtime.providerConfigured
              ? this.snapshot.status
              : "provider-unavailable",
      });
    } catch (error) {
      if (generation !== this.generation || request !== this.providerRequest) return;
      this.update({
        ...this.snapshot,
        providerConfigured: false,
        status: "provider-unavailable",
        error: toErrorReference(error, "assistant_provider_configuration_invalid"),
      });
    }
  };

  readonly stop = (): void => {
    this.generation += 1;
    this.streamGeneration += 1;
    this.submitting = false;
    this.recovering = false;
    const subscription = this.subscription;
    this.subscription = null;
    if (subscription) void subscription.unsubscribe().catch(() => {});
  };

  readonly submit = async (text: string): Promise<void> => {
    const sessionId = this.snapshot.sessionId;
    if (!sessionId || !text || this.isSendDisabled()) return;
    const generation = this.generation;
    this.submitting = true;
    this.update({ ...this.snapshot, isRunning: true, activity: null, error: null });
    try {
      await HarnessService.submitTurn(sessionId, text, getActiveGraphContext()?.graphPath ?? null);
    } catch (error) {
      if (generation !== this.generation || sessionId !== this.snapshot.sessionId) return;
      const failure = toErrorReference(error, "assistant_turn_failed");
      this.update({
        ...this.snapshot,
        activity: null,
        error: failure.code === "harness_turn_cancelled" ? null : failure,
        status: TERMINAL_TURN_ERRORS.has(failure.code) ? this.snapshot.status : "error",
      });
    } finally {
      if (generation === this.generation && sessionId === this.snapshot.sessionId) {
        this.submitting = false;
        this.update({ ...this.snapshot, isRunning: false });
        void this.refreshConversations(generation);
      }
    }
  };

  readonly cancel = async (): Promise<void> => {
    const generation = this.generation;
    if (!this.snapshot.sessionId) return;
    try {
      await HarnessService.cancelTurn(this.snapshot.sessionId);
    } catch (error) {
      if (generation !== this.generation) return;
      const failure = toErrorReference(error, "assistant_turn_failed");
      if (failure.code !== "harness_turn_not_running")
        this.update({ ...this.snapshot, error: failure });
    }
  };

  readonly deleteMemory = async (recordId: string): Promise<void> => {
    if (this.snapshot.sessionId) {
      await HarnessService.deleteMemory(this.snapshot.sessionId, recordId);
    }
  };

  readonly isSendDisabled = (): boolean =>
    this.snapshot.status !== "ready" ||
    !this.snapshot.providerConfigured ||
    !this.snapshot.sessionId ||
    !this.subscription ||
    this.submitting ||
    this.snapshot.isRunning;

  private readonly onEvent = (event: HarnessEvent): void => {
    if (event.sessionId !== this.snapshot.sessionId || event.sequence <= this.snapshot.lastSequence)
      return;
    if (event.sequence !== this.snapshot.lastSequence + 1) {
      void this.recoverStream();
      return;
    }
    this.update(reduceHarnessEvent(this.snapshot, event));
  };

  private readonly onStreamError = (): void => {
    void this.recoverStream();
  };

  private failStream(error: unknown): void {
    this.streamGeneration += 1;
    const subscription = this.subscription;
    this.subscription = null;
    if (subscription) void subscription.unsubscribe().catch(() => {});
    this.update({
      ...this.snapshot,
      status: "error",
      isRunning: false,
      error: toErrorReference(error, "assistant_stream_failed"),
    });
  }

  private async recoverStream(): Promise<void> {
    if (!this.snapshot.sessionId) return;
    if (this.recovering) {
      // A replacement stream must itself be contiguous before it can restore readiness.
      this.failStream(undefined);
      return;
    }
    const generation = this.generation;
    const sessionId = this.snapshot.sessionId;
    this.recovering = true;
    const streamGeneration = ++this.streamGeneration;
    const previous = this.subscription;
    this.subscription = null;
    this.update({ ...this.snapshot, status: "initializing" });
    try {
      await previous?.unsubscribe().catch(() => {});
      if (generation !== this.generation) return;
      const subscription = await HarnessService.subscribeEvents(
        sessionId,
        this.snapshot.lastSequence,
        (event) => {
          if (generation === this.generation && streamGeneration === this.streamGeneration)
            this.onEvent(event);
        },
        () => {
          if (generation === this.generation && streamGeneration === this.streamGeneration)
            this.onStreamError();
        },
      );
      if (generation !== this.generation || streamGeneration !== this.streamGeneration)
        await subscription.unsubscribe();
      else {
        this.subscription = subscription;
        this.update({
          ...this.snapshot,
          status: this.snapshot.providerConfigured ? "ready" : "provider-unavailable",
        });
      }
    } catch (error) {
      if (generation === this.generation) this.failStream(error);
    } finally {
      if (generation === this.generation) this.recovering = false;
    }
  }

  private update(snapshot: AssistantHarnessSnapshot): void {
    this.store.setState(snapshot, true);
  }
}
