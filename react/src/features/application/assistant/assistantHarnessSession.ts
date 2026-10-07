import {
  isModelConfigured,
  modelReasoningEfforts,
  type LanguageModelCatalog,
  type LanguageModelSelection,
} from "@/services/assistant/modelContract";
import { createStore } from "zustand/vanilla";
import { readAssistantDrafts, writeAssistantDrafts } from "./assistantDrafts";
import { reloadAssistantConversations } from "./assistantConversations";
import { toErrorReference } from "@/features/application/errorReference";
import type { ResourceRef } from "@/shared/types/domain/resource";
import { resourceKey } from "@/features/core/resource";
import type { HarnessTurnOptions } from "@/services/assistant/harnessContract";
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
  "assistant_resource_unavailable",
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
  private replaySnapshot: AssistantHarnessSnapshot | null = null;
  private readonly sessions = new Map<string, AssistantHarnessSnapshot>();
  private readonly submissions = new Map<
    string,
    { text: string; resources: readonly ResourceRef[]; accepted: boolean }
  >();

  private get snapshot(): AssistantHarnessSnapshot {
    return this.replaySnapshot ?? this.store.getState();
  }
  private subscription: HarnessEventSubscription | null = null;
  private generation = 0;
  private streamGeneration = 0;
  private modelRequest = 0;
  private modelCatalog: LanguageModelCatalog | null = null;
  private recovering = false;
  readonly start = async (sessionId: string): Promise<void> => {
    this.stop();
    const generation = ++this.generation;
    this.update({ ...INITIAL_SNAPSHOT, status: "initializing" });
    try {
      const session = await HarnessService.openSession(sessionId);
      if (generation !== this.generation) return;
      await this.attachSession(session, generation);
    } catch (error) {
      if (generation === this.generation) this.finishReplay();
      if (generation === this.generation)
        this.update({
          ...this.snapshot,
          status: "error",
          error: toErrorReference(error, "assistant_session_failed"),
        });
    }
  };

  private async attachSession(session: HarnessSession, generation: number): Promise<void> {
    void reloadAssistantConversations();
    const cached = this.sessions.get(session.sessionId);
    const drafts = cached
      ? {
          pending: cached.unsentMessage,
          queued: cached.queuedMessages,
          resources: cached.draftResources,
          options: cached.turnOptions,
        }
      : readAssistantDrafts(session.sessionId);
    this.update({
      ...this.snapshot,
      ...(cached
        ? {
            messages: cached.messages,
            lastSequence: cached.lastSequence,
            isRunning: cached.isRunning,
            isStopping: cached.isStopping,
            activity: cached.activity,
            recoveryAttempt: cached.recoveryAttempt,
            compactionProgress: cached.compactionProgress,
            usage: cached.usage,
          }
        : {}),
      unsentMessage: drafts.pending,
      turnOptions: drafts.options,
      draftResources: drafts.resources,
      queuedMessages: drafts.queued,
      queuePaused: true,
      sessionId: session.sessionId,
      selectedModel: session.model ?? this.modelCatalog?.defaultModel ?? null,
      providerConfigured: isModelConfigured(
        this.modelCatalog,
        session.model ?? this.modelCatalog?.defaultModel ?? null,
      ),
    });
    const streamGeneration = ++this.streamGeneration;
    this.replaySnapshot = this.snapshot;
    const subscription = await HarnessService.subscribeEvents(
      session.sessionId,
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
    if (generation !== this.generation || streamGeneration !== this.streamGeneration) {
      await subscription.unsubscribe();
      return;
    }
    this.subscription = subscription;
    this.finishReplay();
    this.update({
      ...this.snapshot,
      status: this.snapshot.providerConfigured ? "ready" : "provider-unavailable",
    });
  }

  readonly updateModels = (catalog: LanguageModelCatalog | null): void => {
    this.modelCatalog = catalog;
    const selectedModel = this.snapshot.selectedModel ?? catalog?.defaultModel ?? null;
    const providerConfigured = isModelConfigured(catalog, selectedModel);
    const model = catalog?.providers
      .find(({ config }) => config.id === selectedModel?.providerId)
      ?.config.models.find((model) => model.id === selectedModel?.modelId);
    const turnOptions =
      catalog &&
      this.snapshot.turnOptions.reasoningEffort !== null &&
      !modelReasoningEfforts(model)?.includes(this.snapshot.turnOptions.reasoningEffort)
        ? { ...this.snapshot.turnOptions, reasoningEffort: null }
        : this.snapshot.turnOptions;
    this.update({
      ...this.snapshot,
      selectedModel,
      turnOptions,
      providerConfigured,
      status:
        this.subscription && ["ready", "provider-unavailable"].includes(this.snapshot.status)
          ? providerConfigured
            ? "ready"
            : "provider-unavailable"
          : this.snapshot.status,
    });
  };

  readonly selectModel = async (model: LanguageModelSelection): Promise<void> => {
    const sessionId = this.snapshot.sessionId;
    if (!sessionId) return;
    const generation = this.generation;
    const request = ++this.modelRequest;
    this.update({ ...this.snapshot, selectingModel: true });
    try {
      const session = await HarnessService.selectModel(sessionId, model);
      if (generation !== this.generation || request !== this.modelRequest) return;
      this.update({
        ...this.snapshot,
        selectedModel: session.model,
        selectingModel: false,
        error: null,
      });
      this.updateModels(this.modelCatalog);
    } catch (error) {
      if (generation === this.generation && request === this.modelRequest)
        this.update({
          ...this.snapshot,
          selectingModel: false,
          error: toErrorReference(error, "assistant_provider_unavailable"),
        });
    }
  };

  readonly setTurnOptions = (options: HarnessTurnOptions): void => {
    this.update({ ...this.snapshot, turnOptions: { ...options } });
  };

  readonly stop = (): void => {
    this.finishReplay();
    if (this.snapshot.sessionId) {
      this.sessions.delete(this.snapshot.sessionId);
      this.sessions.set(this.snapshot.sessionId, this.snapshot);
    }
    // Only retain recent read projections. Drafts/queued inputs have their own persistence.
    while (this.sessions.size > 3) this.sessions.delete(this.sessions.keys().next().value!);
    this.generation += 1;
    this.streamGeneration += 1;
    this.recovering = false;
    const subscription = this.subscription;
    this.subscription = null;
    if (subscription) void subscription.unsubscribe().catch(() => {});
  };

  readonly submit = async (
    text: string,
    selectedResources?: readonly ResourceRef[],
    model: LanguageModelSelection | null = this.snapshot.selectedModel,
    options: HarnessTurnOptions = this.snapshot.turnOptions,
  ): Promise<boolean> => {
    const sessionId = this.snapshot.sessionId;
    if (!sessionId || !text || this.isSendDisabled()) return false;
    const resources = (selectedResources ?? this.snapshot.draftResources).map((resource) => ({
      ...resource,
    }));
    const fromComposer = selectedResources === undefined;
    const submission = {
      text,
      resources,
      accepted: false,
      afterSequence: this.snapshot.lastSequence,
    };
    this.submissions.set(sessionId, submission);
    const generation = this.generation;
    this.update({
      ...this.snapshot,
      isRunning: true,
      activity: null,
      compactionProgress: null,
      error: null,
      unsentMessage: { text, resources, model, options, afterSequence: this.snapshot.lastSequence },
      draftResources: fromComposer ? [] : this.snapshot.draftResources,
      queuePaused: false,
    });
    try {
      await HarnessService.submitTurn(sessionId, text, resources, model, options);
      submission.accepted = true;
    } catch (error) {
      if (sessionId !== this.snapshot.sessionId) return submission.accepted;
      const failure = toErrorReference(error, "assistant_turn_failed");
      this.update({
        ...this.snapshot,
        activity: null,
        compactionProgress: null,
        error: failure.code === "harness_turn_cancelled" ? null : failure,
        status: TERMINAL_TURN_ERRORS.has(failure.code) ? this.snapshot.status : "error",
        queuePaused: true,
      });
    } finally {
      if (this.submissions.get(sessionId) === submission) this.submissions.delete(sessionId);
      if (
        sessionId === this.snapshot.sessionId &&
        (generation === this.generation || this.subscription)
      ) {
        this.update({
          ...this.snapshot,
          isRunning: false,
          isStopping: false,
          unsentMessage: submission.accepted ? null : this.snapshot.unsentMessage,
          draftResources:
            !submission.accepted && fromComposer && this.snapshot.draftResources.length === 0
              ? resources
              : this.snapshot.draftResources,
        });
        void reloadAssistantConversations();
        this.advanceQueue();
      } else if (submission.accepted) {
        const drafts = readAssistantDrafts(sessionId);
        if (
          drafts.pending?.text === text &&
          drafts.pending.afterSequence === submission.afterSequence
        )
          writeAssistantDrafts(sessionId, null, drafts.queued, drafts.resources, drafts.options);
        const cached = this.sessions.get(sessionId);
        if (cached) this.sessions.set(sessionId, { ...cached, unsentMessage: null });
      }
    }
    return submission.accepted;
  };

  readonly cancel = async (): Promise<void> => {
    const generation = this.generation;
    if (!this.snapshot.sessionId) return;
    this.update({ ...this.snapshot, isStopping: true, error: null, queuePaused: true });
    try {
      await HarnessService.cancelTurn(this.snapshot.sessionId);
    } catch (error) {
      if (generation !== this.generation) return;
      const failure = toErrorReference(error, "assistant_turn_failed");
      if (failure.code !== "harness_turn_not_running")
        this.update({ ...this.snapshot, error: failure, isStopping: false });
      else this.update({ ...this.snapshot, isStopping: false });
    }
  };

  readonly isSendDisabled = (): boolean =>
    this.snapshot.status !== "ready" ||
    !this.snapshot.providerConfigured ||
    this.snapshot.selectingModel ||
    !this.snapshot.sessionId ||
    !this.subscription ||
    this.submissions.has(this.snapshot.sessionId) ||
    this.snapshot.isRunning;

  readonly queueMessage = (text: string): void => {
    if (!text.trim() || !this.snapshot.sessionId) return;
    this.update({
      ...this.snapshot,
      queuedMessages: [
        ...this.snapshot.queuedMessages,
        {
          id: crypto.randomUUID(),
          text,
          resources: [...this.snapshot.draftResources],
          model: this.snapshot.selectedModel,
          options: this.snapshot.turnOptions,
        },
      ],
      draftResources: [],
      queuePaused:
        this.snapshot.isRunning && !this.snapshot.isStopping && this.snapshot.status === "ready"
          ? false
          : this.snapshot.queuePaused,
    });
  };

  readonly removeQueuedMessage = (id: string): void => {
    this.update({
      ...this.snapshot,
      queuedMessages: this.snapshot.queuedMessages.filter((item) => item.id !== id),
    });
  };

  readonly sendNextQueued = async (): Promise<void> => {
    const next = this.snapshot.queuedMessages[0];
    if (!next || this.isSendDisabled()) return;
    this.removeQueuedMessage(next.id);
    await this.submit(next.text, next.resources, next.model, next.options);
  };

  private advanceQueue(): void {
    if (
      !this.replaySnapshot &&
      !this.snapshot.queuePaused &&
      !this.snapshot.error &&
      this.snapshot.messages[this.snapshot.messages.length - 1]?.status.type === "complete"
    )
      void this.sendNextQueued();
  }

  readonly restoreUnsentMessage = (): void => {
    const pending = this.snapshot.unsentMessage;
    const current = this.snapshot.draftResources;
    const keys = new Set(current.map(resourceKey));
    const restored =
      this.snapshot.unsentMessage?.resources.filter(
        (resource) => !keys.has(resourceKey(resource)),
      ) ?? [];
    this.update({
      ...this.snapshot,
      draftResources: [...current, ...restored],
      turnOptions: pending?.options ?? this.snapshot.turnOptions,
      unsentMessage: null,
    });
    if (pending?.model) void this.selectModel(pending.model);
  };

  readonly addResource = (resource: ResourceRef): void => {
    if (
      this.snapshot.draftResources.some(
        (existing) => resourceKey(existing) === resourceKey(resource),
      )
    )
      return;
    this.update({
      ...this.snapshot,
      draftResources: [...this.snapshot.draftResources, { ...resource }],
    });
  };

  readonly removeResource = (resource: ResourceRef): void => {
    this.update({
      ...this.snapshot,
      draftResources: this.snapshot.draftResources.filter(
        (existing) => resourceKey(existing) !== resourceKey(resource),
      ),
    });
  };

  private readonly onEvent = (event: HarnessEvent): void => {
    if (event.sessionId !== this.snapshot.sessionId || event.sequence <= this.snapshot.lastSequence)
      return;
    if (event.sequence !== this.snapshot.lastSequence + 1) {
      void this.recoverStream();
      return;
    }
    if (event.type === "turn_started") {
      const pending = this.submissions.get(event.sessionId);
      if (
        pending?.text === event.payload.userMessage &&
        sameResources(
          pending.resources,
          event.payload.resources.map((entry) => entry.resource),
        )
      )
        pending.accepted = true;
      if (
        this.snapshot.unsentMessage &&
        event.sequence > this.snapshot.unsentMessage.afterSequence &&
        event.payload.userMessage === this.snapshot.unsentMessage.text &&
        sameResources(
          this.snapshot.unsentMessage.resources,
          event.payload.resources.map((entry) => entry.resource),
        )
      )
        this.update({ ...this.snapshot, unsentMessage: null });
    }
    this.update(reduceHarnessEvent(this.snapshot, event));
    if (event.type === "turn_completed") this.advanceQueue();
  };

  private readonly onStreamError = (): void => {
    void this.recoverStream();
  };

  private failStream(error: unknown): void {
    this.finishReplay();
    this.streamGeneration += 1;
    const subscription = this.subscription;
    this.subscription = null;
    if (subscription) void subscription.unsubscribe().catch(() => {});
    this.update({
      ...this.snapshot,
      status: "error",
      isRunning: false,
      isStopping: false,
      activity: null,
      compactionProgress: null,
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
    this.finishReplay();
    this.update({
      ...this.snapshot,
      status: "initializing",
      activity: null,
      compactionProgress: null,
    });
    try {
      await previous?.unsubscribe().catch(() => {});
      if (generation !== this.generation) return;
      this.replaySnapshot = this.snapshot;
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
        this.finishReplay();
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
    snapshot = {
      ...snapshot,
      isSubmitting: snapshot.sessionId !== null && this.submissions.has(snapshot.sessionId),
    };
    if (
      snapshot.sessionId &&
      (snapshot.unsentMessage !== this.snapshot.unsentMessage ||
        snapshot.queuedMessages !== this.snapshot.queuedMessages ||
        snapshot.draftResources !== this.snapshot.draftResources ||
        snapshot.turnOptions !== this.snapshot.turnOptions)
    )
      writeAssistantDrafts(
        snapshot.sessionId,
        snapshot.unsentMessage,
        snapshot.queuedMessages,
        snapshot.draftResources,
        snapshot.turnOptions,
      );
    if (this.replaySnapshot) {
      this.replaySnapshot = snapshot;
      return;
    }
    this.store.setState(snapshot, true);
  }

  private finishReplay(): void {
    const snapshot = this.replaySnapshot;
    this.replaySnapshot = null;
    if (snapshot) this.store.setState(snapshot, true);
  }
}

function sameResources(left: readonly ResourceRef[], right: readonly ResourceRef[]): boolean {
  return (
    left.length === right.length &&
    left.every((resource, index) => resourceKey(resource) === resourceKey(right[index]))
  );
}
