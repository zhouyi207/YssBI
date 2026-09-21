import {
  appendAssistantText,
  finishAssistantText,
  updateAssistantContent,
  type AssistantMessageContent,
  type ProjectionToolCall,
} from "./assistantMessageContent";
import {
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
} from "@assistant-ui/react";
import { useEffect, useLayoutEffect, useRef, useSyncExternalStore } from "react";

import { getSettingsSnapshot, useSettingsRead } from "@/features/core/settings/read";
import { toErrorReference, type ErrorReference } from "@/features/application/errorReference";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { getActiveGraphContext } from "@/features/application/editor/editorGroupContext";
import {
  HarnessService,
  type HarnessEvent,
  type HarnessSession,
  type HarnessEventSubscription,
  type HarnessKnowledgeCitation,
  type HarnessMemoryRecord,
} from "@/services/assistant/harnessService";

type ProjectionStatus = "initializing" | "ready" | "provider-unavailable" | "error" | "closed";
type ProjectionMessageStatus =
  | Readonly<{ type: "running" }>
  | Readonly<{ type: "complete"; reason: "stop" }>
  | Readonly<{ type: "incomplete"; reason: "cancelled" | "error" }>;

interface ProjectionMessage {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly content: AssistantMessageContent;
  readonly createdAt: Date;
  readonly status: ProjectionMessageStatus;
}

export interface AssistantHarnessSnapshot {
  readonly status: ProjectionStatus;
  readonly error: ErrorReference | null;
  readonly providerConfigured: boolean;
  readonly sessionId: string | null;
  readonly conversations: readonly HarnessSession[];
  readonly lastSequence: number;
  readonly messages: readonly ProjectionMessage[];
  readonly isRunning: boolean;
  readonly activity: string | null;
  readonly memoryCount: number;
  readonly memoryRecords: readonly HarnessMemoryRecord[];
}

const INITIAL_SNAPSHOT: AssistantHarnessSnapshot = Object.freeze({
  status: "initializing",
  error: null,
  providerConfigured: false,
  sessionId: null,
  conversations: Object.freeze([]),
  lastSequence: 0,
  messages: Object.freeze([]),
  isRunning: false,
  activity: null,
  memoryCount: 0,
  memoryRecords: Object.freeze([]),
});

const TERMINAL_TURN_ERRORS = new Set([
  "assistant_provider_unavailable",
  "assistant_authentication_failed",
  "assistant_rate_limited",
  "assistant_provider_request_rejected",
  "assistant_context_window_exceeded",
  "assistant_provider_connection_failed",
  "assistant_invalid_provider_response",
  "assistant_turn_failed",
  "assistant_turn_timed_out",
  "harness_turn_cancelled",
  "invalid_harness_request",
]);

function convertProjectionMessage(message: ProjectionMessage): ThreadMessageLike {
  return {
    id: message.id,
    role: message.role,
    content: message.content,
    createdAt: message.createdAt,
    ...(message.role === "assistant" ? { status: message.status } : {}),
  };
}

function appendedText(message: AppendMessage): string {
  return message.content
    .filter(
      (part): part is Extract<(typeof message.content)[number], { type: "text" }> =>
        part.type === "text",
    )
    .map((part) => part.text)
    .join("\n")
    .trim();
}

class AssistantHarnessProjection {
  private snapshot: AssistantHarnessSnapshot = INITIAL_SNAPSHOT;
  private readonly listeners = new Set<() => void>();
  private subscription: HarnessEventSubscription | null = null;
  private generation = 0;
  private streamGeneration = 0;
  private recovering = false;
  private submitting = false;
  private readonly citationsByTurn = new Map<string, HarnessKnowledgeCitation[]>();
  private readonly plansByTurn = new Map<string, unknown>();
  private readonly toolsByTurn = new Map<string, ProjectionToolCall[]>();

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  readonly getSnapshot = (): AssistantHarnessSnapshot => this.snapshot;

  readonly start = async (): Promise<void> => {
    this.stop();
    const generation = ++this.generation;
    this.citationsByTurn.clear();
    this.plansByTurn.clear();
    this.toolsByTurn.clear();
    this.update({ ...INITIAL_SNAPSHOT, status: "initializing" });
    try {
      const settings = getSettingsSnapshot();
      if (!settings.isLoading) {
        await HarnessService.configureProvider(
          settings.ai.openAiModel,
          settings.ai.openAiBaseUrl,
          settings.ai.openAiApiKey,
        ).catch((error: unknown) => {
          if (generation === this.generation)
            this.update({
              ...this.snapshot,
              error: toErrorReference(error, "assistant_provider_configuration_invalid"),
            });
        });
      }
      if (generation !== this.generation) return;
      const runtime = await HarnessService.runtimeStatus();
      if (generation !== this.generation) return;
      const conversations = await HarnessService.listSessions();
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        conversations,
        providerConfigured: runtime.providerConfigured,
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
    this.citationsByTurn.clear();
    this.plansByTurn.clear();
    this.toolsByTurn.clear();
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
    const memoryRecords = await HarnessService.listMemory(session.sessionId);
    if (generation !== this.generation) return;
    this.update({
      ...this.snapshot,
      memoryRecords,
      memoryCount: memoryRecords.length,
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

  readonly syncProvider = async (model: string, baseUrl: string, apiKey: string): Promise<void> => {
    const generation = this.generation;
    try {
      const runtime = await HarnessService.configureProvider(model, baseUrl, apiKey);
      if (generation !== this.generation) return;
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
      if (generation !== this.generation) return;
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

  readonly submit = async (message: AppendMessage): Promise<void> => {
    const sessionId = this.snapshot.sessionId;
    const text = appendedText(message);
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
    this.submitting ||
    this.snapshot.isRunning;

  private readonly onEvent = (event: HarnessEvent): void => {
    if (event.sessionId !== this.snapshot.sessionId || event.sequence <= this.snapshot.lastSequence)
      return;
    if (event.sequence !== this.snapshot.lastSequence + 1) {
      void this.recoverStream();
      return;
    }
    this.applyEvent(event);
  };

  private readonly onStreamError = (): void => {
    void this.recoverStream();
  };

  private async recoverStream(): Promise<void> {
    if (this.recovering || !this.snapshot.sessionId) return;
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
      if (generation === this.generation)
        this.update({
          ...this.snapshot,
          status: "error",
          isRunning: false,
          error: toErrorReference(error, "assistant_stream_failed"),
        });
    } finally {
      if (generation === this.generation) this.recovering = false;
    }
  }

  private applyEvent(event: HarnessEvent): void {
    let messages = this.snapshot.messages;
    let isRunning = this.snapshot.isRunning;
    let activity = this.snapshot.activity;
    let status = this.snapshot.status;
    let memoryCount = this.snapshot.memoryCount;
    let memoryRecords = this.snapshot.memoryRecords;
    if (event.type === "turn_started" && event.turnId) {
      messages = [
        ...messages,
        {
          id: `user-${event.turnId}`,
          role: "user",
          content: [{ type: "text", text: event.payload.userMessage }],
          createdAt: new Date(event.occurredAt),
          status: { type: "complete", reason: "stop" },
        },
      ];
      isRunning = true;
    } else if (event.type === "text_delta" && event.turnId) {
      messages = upsertAssistantMessage(
        messages,
        event.turnId,
        event.payload.delta,
        event.occurredAt,
        this.citationsByTurn.get(event.turnId) ?? [],
        this.plansByTurn.get(event.turnId) ?? null,
        this.toolsByTurn.get(event.turnId) ?? [],
      );
      isRunning = true;
    } else if (event.type === "turn_completed" && event.turnId) {
      this.settleTurnTools(event.turnId, "interrupted");
      messages = completeAssistantMessage(
        messages,
        event.turnId,
        event.payload.finalText,
        event.occurredAt,
        this.citationsByTurn.get(event.turnId) ?? [],
        this.plansByTurn.get(event.turnId) ?? null,
        this.toolsByTurn.get(event.turnId) ?? [],
      );
      isRunning = false;
      activity = null;
    } else if ((event.type === "turn_failed" || event.type === "turn_cancelled") && event.turnId) {
      this.settleTurnTools(
        event.turnId,
        event.type === "turn_cancelled" ? "cancelled" : "interrupted",
      );
      messages = failAssistantMessage(
        messages,
        event.turnId,
        event.type === "turn_cancelled" ? "cancelled" : "error",
        event.occurredAt,
        this.citationsByTurn.get(event.turnId) ?? [],
        this.plansByTurn.get(event.turnId) ?? null,
        this.toolsByTurn.get(event.turnId) ?? [],
      );
      isRunning = false;
      activity = null;
    } else if (event.type === "tool_invocation_requested" && event.turnId) {
      const turnId = event.turnId;
      const tools = [
        ...(this.toolsByTurn.get(turnId) ?? []),
        {
          invocationId: `pending-${event.sequence}`,
          capabilityId: event.payload.capabilityId,
          state: "running" as const,
        },
      ];
      this.toolsByTurn.set(turnId, tools);
      messages = upsertToolMessage(
        messages,
        turnId,
        event.occurredAt,
        this.citationsByTurn.get(turnId) ?? [],
        this.plansByTurn.get(turnId) ?? null,
        tools,
      );
      activity = event.payload.capabilityId;
    } else if (
      (event.type === "tool_invocation_started" ||
        event.type === "tool_invocation_completed" ||
        event.type === "tool_invocation_failed") &&
      event.turnId
    ) {
      const turnId = event.turnId;
      const failureCode =
        event.type === "tool_invocation_failed" ? event.payload.failureCode : null;
      const state: ProjectionToolCall["state"] =
        event.type === "tool_invocation_started"
          ? "running"
          : event.type === "tool_invocation_completed"
            ? "completed"
            : failureCode === "cancelled"
              ? "cancelled"
              : failureCode === "outcome_unknown"
                ? "unknown"
                : failureCode === "deadline_elapsed"
                  ? "timed-out"
                  : "failed";
      const tools = [...(this.toolsByTurn.get(turnId) ?? [])];
      let index = tools.findIndex((tool) => tool.invocationId === event.payload.invocationId);
      // Older persisted streams contain a requested placeholder before the identified event.
      if (index < 0)
        index = tools.findIndex(
          (tool) =>
            tool.invocationId.startsWith("pending-") &&
            tool.state === "running" &&
            tool.capabilityId === event.payload.capabilityId,
        );
      const tool = {
        invocationId: event.payload.invocationId,
        capabilityId: event.payload.capabilityId,
        state,
      };
      if (index < 0) tools.push(tool);
      else tools[index] = tool;
      this.toolsByTurn.set(turnId, tools);
      messages = upsertToolMessage(
        messages,
        turnId,
        event.occurredAt,
        this.citationsByTurn.get(turnId) ?? [],
        this.plansByTurn.get(turnId) ?? null,
        tools,
      );
      activity = tools.find((tool) => tool.state === "running")?.capabilityId ?? null;
    } else if (event.type === "workflow_started") {
      activity = event.payload.runId;
    } else if (
      event.type === "workflow_completed" ||
      event.type === "workflow_paused" ||
      event.type === "workflow_cancelled"
    ) {
      activity = null;
    } else if (event.type === "workflow_resumed") {
      activity = event.payload.runId;
    } else if (event.type === "knowledge_cited" && event.turnId) {
      const turnId = event.turnId;
      const citations = this.citationsByTurn.get(turnId) ?? [];
      if (!citations.some((citation) => citation.chunkId === event.payload.citation.chunkId)) {
        this.citationsByTurn.set(turnId, [...citations, event.payload.citation]);
      }
      messages = upsertToolMessage(
        messages,
        turnId,
        event.occurredAt,
        this.citationsByTurn.get(turnId) ?? [],
        this.plansByTurn.get(turnId) ?? null,
        this.toolsByTurn.get(turnId) ?? [],
      );
    } else if (event.type === "plan_proposed" && event.turnId) {
      const turnId = event.turnId;
      this.plansByTurn.set(turnId, event.payload.plan);
      messages = upsertPlanMessage(
        messages,
        turnId,
        event.payload.plan,
        event.occurredAt,
        this.citationsByTurn.get(turnId) ?? [],
        this.toolsByTurn.get(turnId) ?? [],
      );
    } else if (event.type === "memory_recorded") {
      memoryRecords = [
        ...memoryRecords.filter((record) => record.recordId !== event.payload.record.recordId),
        event.payload.record,
      ];
      memoryCount = memoryRecords.length;
    } else if (event.type === "memory_deleted") {
      memoryRecords = memoryRecords.filter((record) => record.recordId !== event.payload.recordId);
      memoryCount = memoryRecords.length;
    } else if (event.type === "session_closed") {
      status = "closed";
      isRunning = false;
    }
    this.update({
      ...this.snapshot,
      lastSequence: event.sequence,
      messages,
      isRunning,
      activity,
      status,
      memoryCount,
      memoryRecords,
    });
  }

  private settleTurnTools(turnId: string, state: "cancelled" | "interrupted"): void {
    this.toolsByTurn.set(
      turnId,
      (this.toolsByTurn.get(turnId) ?? []).map((tool) =>
        tool.state === "running" ? { ...tool, state } : tool,
      ),
    );
  }

  private update(snapshot: AssistantHarnessSnapshot): void {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener();
  }
}

function updateAssistantMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
  transform: (content: AssistantMessageContent) => AssistantMessageContent,
  status?: ProjectionMessageStatus,
): readonly ProjectionMessage[] {
  const id = `assistant-${turnId}`;
  const existing = messages.find((message) => message.id === id);
  const message: ProjectionMessage = {
    id,
    role: "assistant",
    createdAt: existing?.createdAt ?? new Date(occurredAt),
    content: transform(updateAssistantContent(existing?.content ?? [], citations, plan, tools)),
    status: status ?? existing?.status ?? { type: "running" },
  };
  return existing
    ? messages.map((previous) => (previous.id === id ? message : previous))
    : [...messages, message];
}

function upsertAssistantMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  delta: string,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  return updateAssistantMessage(
    messages,
    turnId,
    occurredAt,
    citations,
    plan,
    tools,
    (content) => appendAssistantText(content, delta),
    { type: "running" },
  );
}

function completeAssistantMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  finalText: string,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  return updateAssistantMessage(
    messages,
    turnId,
    occurredAt,
    citations,
    plan,
    tools,
    (content) => finishAssistantText(content, finalText),
    { type: "complete", reason: "stop" },
  );
}

function failAssistantMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  reason: "cancelled" | "error",
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  return updateAssistantMessage(
    messages,
    turnId,
    occurredAt,
    citations,
    plan,
    tools,
    (content) => content,
    { type: "incomplete", reason },
  );
}

function upsertPlanMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  plan: unknown,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  return updateAssistantMessage(
    messages,
    turnId,
    occurredAt,
    citations,
    plan,
    tools,
    (content) => content,
  );
}

function upsertToolMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  return updateAssistantMessage(
    messages,
    turnId,
    occurredAt,
    citations,
    plan,
    tools,
    (content) => content,
  );
}

export function useAssistantHarnessRuntime() {
  const projectionRef = useRef<AssistantHarnessProjection | null>(null);
  projectionRef.current ??= new AssistantHarnessProjection();
  const projection = projectionRef.current;
  const ai = useSettingsRead((state) => state.ai);
  const projectInstanceId = useProjectIOStore((state) => state.projectInstanceId);
  const isLoading = useSettingsRead((state) => state.isLoading);
  const snapshot = useSyncExternalStore(
    projection.subscribe,
    projection.getSnapshot,
    projection.getSnapshot,
  );
  useEffect(() => {
    void projection.start();
    return projection.stop;
  }, [projection, projectInstanceId]);
  useEffect(() => {
    if (isLoading) return;
    const timer = window.setTimeout(() => {
      void projection.syncProvider(ai.openAiModel, ai.openAiBaseUrl, ai.openAiApiKey);
    }, 300);
    return () => window.clearTimeout(timer);
  }, [ai.openAiApiKey, ai.openAiBaseUrl, ai.openAiModel, isLoading, projection]);
  const runtime = useExternalStoreRuntime({
    messages: snapshot.messages,
    convertMessage: convertProjectionMessage,
    isRunning: snapshot.isRunning,
    isSendDisabled: projection.isSendDisabled(),
    onNew: projection.submit,
    onCancel: projection.cancel,
  });
  const drafts = useRef(new Map<string, string>());
  const draftSessionId = useRef<string | null>(null);
  useLayoutEffect(() => {
    if (draftSessionId.current === snapshot.sessionId) return;
    if (draftSessionId.current)
      drafts.current.set(draftSessionId.current, runtime.thread.composer.getState().text);
    runtime.thread.composer.setText(
      snapshot.sessionId ? (drafts.current.get(snapshot.sessionId) ?? "") : "",
    );
    draftSessionId.current = snapshot.sessionId;
  }, [runtime, snapshot.sessionId]);
  return {
    runtime,
    snapshot,
    deleteMemory: projection.deleteMemory,
    newConversation: projection.newConversation,
    selectConversation: projection.selectConversation,
    reloadConversations: projection.start,
  };
}
