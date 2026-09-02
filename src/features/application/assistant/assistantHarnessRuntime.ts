import {
  useExternalStoreRuntime,
  type AppendMessage,
  type ThreadMessageLike,
} from "@assistant-ui/react";
import { useEffect, useRef, useSyncExternalStore } from "react";

import { getSettingsSnapshot, useSettingsRead } from "@/features/core/settings/read";
import {
  HarnessService,
  type HarnessEvent,
  type HarnessEventSubscription,
  type HarnessKnowledgeCitation,
  type HarnessMemoryRecord,
} from "@/services/assistant/harnessService";

type ProjectionStatus = "initializing" | "ready" | "provider-unavailable" | "error" | "closed";
type ProjectionMessageStatus =
  | Readonly<{ type: "running" }>
  | Readonly<{ type: "complete"; reason: "stop" }>
  | Readonly<{ type: "incomplete"; reason: "cancelled" | "error" }>;

interface ProjectionToolCall {
  readonly invocationId: string;
  readonly capabilityId: string;
  readonly completed: boolean;
}

interface ProjectionMessage {
  readonly id: string;
  readonly role: "user" | "assistant";
  readonly text: string;
  readonly createdAt: Date;
  readonly status: ProjectionMessageStatus;
  readonly citations: readonly HarnessKnowledgeCitation[];
  readonly plan: unknown | null;
  readonly tools: readonly ProjectionToolCall[];
}

export interface AssistantHarnessSnapshot {
  readonly status: ProjectionStatus;
  readonly providerConfigured: boolean;
  readonly sessionId: string | null;
  readonly lastSequence: number;
  readonly messages: readonly ProjectionMessage[];
  readonly isRunning: boolean;
  readonly activity: string | null;
  readonly memoryCount: number;
  readonly memoryRecords: readonly HarnessMemoryRecord[];
}

const INITIAL_SNAPSHOT: AssistantHarnessSnapshot = Object.freeze({
  status: "initializing",
  providerConfigured: false,
  sessionId: null,
  lastSequence: 0,
  messages: Object.freeze([]),
  isRunning: false,
  activity: null,
  memoryCount: 0,
  memoryRecords: Object.freeze([]),
});

function convertProjectionMessage(message: ProjectionMessage): ThreadMessageLike {
  return {
    id: message.id,
    role: message.role,
    content: [
      { type: "text", text: message.text },
      ...message.citations.map((citation) => ({
        type: "source" as const,
        sourceType: "document" as const,
        id: citation.chunkId,
        title: citation.title,
        mediaType: "text/markdown",
      })),
      ...(message.plan
        ? [{ type: "data" as const, name: "statistical-plan", data: message.plan }]
        : []),
      ...message.tools.map((tool) => ({
        type: "tool-call" as const,
        toolCallId: tool.invocationId,
        toolName: tool.capabilityId,
        args: {},
        ...(tool.completed ? { result: { status: "completed" } } : {}),
      })),
    ],
    createdAt: message.createdAt,
    status: message.status,
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
  private recovering = false;
  private readonly citationsByTurn = new Map<string, HarnessKnowledgeCitation[]>();
  private readonly plansByTurn = new Map<string, unknown>();
  private readonly toolsByTurn = new Map<string, ProjectionToolCall[]>();

  readonly subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => this.listeners.delete(listener);
  };

  readonly getSnapshot = (): AssistantHarnessSnapshot => this.snapshot;

  readonly start = async (): Promise<void> => {
    const generation = ++this.generation;
    this.update({ ...INITIAL_SNAPSHOT, status: "initializing" });
    try {
      const settings = getSettingsSnapshot();
      if (!settings.isLoading) {
        await HarnessService.configureProvider(
          settings.ai.openAiModel,
          settings.ai.openAiBaseUrl,
          settings.ai.openAiApiKey,
        ).catch(() => undefined);
      }
      if (generation !== this.generation) return;
      const [runtime, session] = await Promise.all([
        HarnessService.runtimeStatus(),
        HarnessService.createSession(),
      ]);
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        providerConfigured: runtime.providerConfigured,
        sessionId: session.sessionId,
        status: runtime.providerConfigured ? "ready" : "provider-unavailable",
      });
      this.subscription = await HarnessService.subscribeEvents(
        session.sessionId,
        0,
        this.onEvent,
        this.onStreamError,
      );
      if (generation !== this.generation) await this.subscription.unsubscribe();
    } catch {
      if (generation === this.generation) this.update({ ...this.snapshot, status: "error" });
    }
  };

  readonly syncProvider = async (model: string, baseUrl: string, apiKey: string): Promise<void> => {
    const generation = this.generation;
    try {
      const runtime = await HarnessService.configureProvider(model, baseUrl, apiKey);
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        providerConfigured: runtime.providerConfigured,
        status:
          runtime.providerConfigured && this.snapshot.status !== "closed"
            ? "ready"
            : runtime.providerConfigured
              ? this.snapshot.status
              : "provider-unavailable",
      });
    } catch {
      if (generation !== this.generation) return;
      this.update({
        ...this.snapshot,
        providerConfigured: false,
        status: "provider-unavailable",
      });
    }
  };

  readonly stop = (): void => {
    this.generation += 1;
    const subscription = this.subscription;
    const sessionId = this.snapshot.sessionId;
    this.subscription = null;
    if (subscription) void subscription.unsubscribe().catch(() => {});
    if (sessionId) void HarnessService.closeSession(sessionId).catch(() => {});
  };

  readonly submit = async (message: AppendMessage): Promise<void> => {
    const sessionId = this.snapshot.sessionId;
    const text = appendedText(message);
    if (!sessionId || !text || this.isSendDisabled()) return;
    this.update({ ...this.snapshot, isRunning: true, activity: null });
    try {
      await HarnessService.submitTurn(sessionId, text);
    } catch {
      this.update({ ...this.snapshot, isRunning: false, status: "error" });
    }
  };

  readonly cancel = async (): Promise<void> => {
    if (this.snapshot.sessionId) await HarnessService.cancelTurn(this.snapshot.sessionId);
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
    this.recovering = true;
    try {
      await this.subscription?.unsubscribe();
      this.subscription = await HarnessService.subscribeEvents(
        this.snapshot.sessionId,
        this.snapshot.lastSequence,
        this.onEvent,
        this.onStreamError,
      );
    } catch {
      this.update({ ...this.snapshot, status: "error", isRunning: false });
    } finally {
      this.recovering = false;
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
          text: event.payload.userMessage,
          createdAt: new Date(event.occurredAt),
          status: { type: "complete", reason: "stop" },
          citations: [],
          plan: null,
          tools: [],
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
          completed: false,
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
    } else if (event.type === "tool_invocation_started" && event.turnId) {
      const turnId = event.turnId;
      const tools = [
        ...(this.toolsByTurn.get(turnId) ?? []),
        {
          invocationId: event.payload.invocationId,
          capabilityId: event.payload.capabilityId,
          completed: false,
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
    } else if (event.type === "tool_invocation_completed" && event.turnId) {
      const turnId = event.turnId;
      let matched = false;
      const tools = (this.toolsByTurn.get(turnId) ?? []).map((tool) => {
        if (!matched && !tool.completed && tool.capabilityId === event.payload.capabilityId) {
          matched = true;
          return { ...tool, invocationId: event.payload.invocationId, completed: true };
        }
        return tool;
      });
      if (!matched) {
        tools.push({
          invocationId: event.payload.invocationId,
          capabilityId: event.payload.capabilityId,
          completed: true,
        });
      }
      this.toolsByTurn.set(turnId, tools);
      messages = upsertToolMessage(
        messages,
        turnId,
        event.occurredAt,
        this.citationsByTurn.get(turnId) ?? [],
        this.plansByTurn.get(turnId) ?? null,
        tools,
      );
      activity = null;
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
      messages = messages.map((message) =>
        message.id === `assistant-${turnId}`
          ? { ...message, citations: this.citationsByTurn.get(turnId) ?? [] }
          : message,
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

  private update(snapshot: AssistantHarnessSnapshot): void {
    this.snapshot = snapshot;
    for (const listener of this.listeners) listener();
  }
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
  const id = `assistant-${turnId}`;
  const existing = messages.find((message) => message.id === id);
  if (!existing) {
    return [
      ...messages,
      {
        id,
        role: "assistant",
        text: delta,
        createdAt: new Date(occurredAt),
        status: { type: "running" },
        citations,
        plan,
        tools,
      },
    ];
  }
  return messages.map((message) =>
    message.id === id
      ? {
          ...message,
          text: `${message.text}${delta}`,
          status: { type: "running" },
          citations,
          plan,
          tools,
        }
      : message,
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
  const id = `assistant-${turnId}`;
  if (!messages.some((message) => message.id === id)) {
    return [
      ...messages,
      {
        id,
        role: "assistant",
        text: finalText,
        createdAt: new Date(occurredAt),
        status: { type: "complete", reason: "stop" },
        citations,
        plan,
        tools,
      },
    ];
  }
  return messages.map((message) =>
    message.id === id
      ? {
          ...message,
          text: finalText,
          status: { type: "complete", reason: "stop" },
          citations,
          plan,
          tools,
        }
      : message,
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
  const id = `assistant-${turnId}`;
  if (!messages.some((message) => message.id === id)) {
    return [
      ...messages,
      {
        id,
        role: "assistant",
        text: " ",
        createdAt: new Date(occurredAt),
        status: { type: "incomplete", reason },
        citations,
        plan,
        tools,
      },
    ];
  }
  return messages.map((message) =>
    message.id === id
      ? { ...message, status: { type: "incomplete", reason }, citations, plan, tools }
      : message,
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
  const id = `assistant-${turnId}`;
  if (!messages.some((message) => message.id === id)) {
    return [
      ...messages,
      {
        id,
        role: "assistant",
        text: "",
        createdAt: new Date(occurredAt),
        status: { type: "running" },
        citations,
        plan,
        tools,
      },
    ];
  }
  return messages.map((message) => (message.id === id ? { ...message, plan } : message));
}

function upsertToolMessage(
  messages: readonly ProjectionMessage[],
  turnId: string,
  occurredAt: number,
  citations: readonly HarnessKnowledgeCitation[],
  plan: unknown | null,
  tools: readonly ProjectionToolCall[],
): readonly ProjectionMessage[] {
  const id = `assistant-${turnId}`;
  if (!messages.some((message) => message.id === id)) {
    return [
      ...messages,
      {
        id,
        role: "assistant",
        text: "",
        createdAt: new Date(occurredAt),
        status: { type: "running" },
        citations,
        plan,
        tools,
      },
    ];
  }
  return messages.map((message) => (message.id === id ? { ...message, tools } : message));
}

export function useAssistantHarnessRuntime() {
  const projectionRef = useRef<AssistantHarnessProjection | null>(null);
  projectionRef.current ??= new AssistantHarnessProjection();
  const projection = projectionRef.current;
  const ai = useSettingsRead((state) => state.ai);
  const isLoading = useSettingsRead((state) => state.isLoading);
  const snapshot = useSyncExternalStore(
    projection.subscribe,
    projection.getSnapshot,
    projection.getSnapshot,
  );
  useEffect(() => {
    void projection.start();
    return projection.stop;
  }, [projection]);
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
  return { runtime, snapshot, deleteMemory: projection.deleteMemory };
}
