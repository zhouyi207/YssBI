import { testModelCatalog, testModelIdentity } from "@/tests/fixtures/harnessModels";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { HarnessService, type HarnessEvent } from "@/services/assistant/harnessService";
import { AssistantHarnessProjection } from "./assistantHarnessSession";

vi.mock("@/features/application/editor/editorGroupContext", () => ({
  getActiveGraphContext: () => null,
}));
vi.mock("@/services/assistant/harnessService", () => ({
  HarnessService: {
    selectModel: vi.fn(),
    createSession: vi.fn(),
    subscribeEvents: vi.fn(),
    submitTurn: vi.fn(),
    cancelTurn: vi.fn(),
    openSession: vi.fn(),
  },
}));

const session = {
  sessionId: "session-1",
  projectInstanceId: "project-1",
  projectSessionId: "project-session-1",
  title: "",
  lastOpenedAt: 0,
  model: null,
};

function event(sequence: number): HarnessEvent {
  return {
    sessionId: session.sessionId,
    sequence,
    turnId: null,
    occurredAt: 1000,
    type: "session_created",
  };
}

beforeEach(() => {
  const drafts = new Map<string, string>();
  vi.stubGlobal("localStorage", {
    getItem: (key: string) => drafts.get(key) ?? null,
    setItem: (key: string, value: string) => drafts.set(key, value),
    removeItem: (key: string) => drafts.delete(key),
  });
  vi.resetAllMocks();
  vi.mocked(HarnessService.openSession).mockResolvedValue(session);
});
afterEach(() => vi.unstubAllGlobals());

it("keeps rejected input without treating an accepted failing turn as an unsent draft", async () => {
  let onEvent!: (event: HarnessEvent) => void;
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, _after, callback) => {
      onEvent = callback;
      callback(event(1));
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  const projection = new AssistantHarnessProjection();
  await projection.start(session.sessionId);
  projection.updateModels(testModelCatalog);
  vi.mocked(HarnessService.submitTurn).mockRejectedValueOnce({
    code: "assistant_provider_unavailable",
    details: {},
    incidentId: null,
  });
  expect(await projection.submit("Do not lose this text")).toBe(false);
  expect(projection.getState()).toMatchObject({
    isRunning: false,
    unsentMessage: { text: "Do not lose this text", afterSequence: 1 },
  });
  vi.mocked(HarnessService.submitTurn).mockImplementationOnce(async () => {
    onEvent({
      ...event(2),
      turnId: "turn-1",
      type: "turn_started",
      payload: { model: testModelIdentity, resources: [], userMessage: "Accepted text" },
    });
    onEvent({ ...event(3), turnId: "turn-1", type: "turn_failed" });
    throw { code: "assistant_turn_failed", details: {}, incidentId: null };
  });
  expect(await projection.submit("Accepted text")).toBe(true);
  expect(projection.getState().unsentMessage).toBeNull();
  expect(projection.getState().messages[0].content).toEqual([
    { type: "text", text: "Accepted text" },
  ]);
  projection.stop();
});

it("pauses queued follow-ups on cancellation even when the in-flight task completes", async () => {
  let onEvent!: (event: HarnessEvent) => void;
  let finish!: () => void;
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, _after, callback) => {
      onEvent = callback;
      callback(event(1));
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  vi.mocked(HarnessService.submitTurn).mockImplementationOnce(async () => {
    onEvent({
      ...event(2),
      turnId: "turn-1",
      type: "turn_started",
      payload: { model: testModelIdentity, resources: [], userMessage: "Run" },
    });
    await new Promise<void>((resolve) => {
      finish = resolve;
    });
    onEvent({
      ...event(3),
      turnId: "turn-1",
      type: "turn_completed",
      payload: { finalText: "Finished" },
    });
    return { finalText: "Finished" };
  });
  vi.mocked(HarnessService.cancelTurn).mockResolvedValue(undefined);
  const projection = new AssistantHarnessProjection();
  await projection.start(session.sessionId);
  projection.updateModels(testModelCatalog);
  const pending = projection.submit("Run");
  const originalResource = { kind: "event_graph" as const, id: "events/original.yssbi-event" };
  projection.addResource(originalResource);
  projection.queueMessage("Follow up");
  await projection.cancel();
  finish();
  await pending;
  expect(HarnessService.submitTurn).toHaveBeenCalledTimes(1);
  expect(projection.getState()).toMatchObject({
    queuePaused: true,
    queuedMessages: [{ text: "Follow up", resources: [originalResource] }],
  });
  const nextModel = { ...testModelIdentity.selection, modelId: "next-model" };
  projection.updateModels({
    ...testModelCatalog,
    providers: testModelCatalog.providers.map((provider) => ({
      ...provider,
      config: {
        ...provider.config,
        models: [
          ...provider.config.models,
          {
            id: nextModel.modelId,
            name: "Next model",
            contextWindow: null,
            maxOutputTokens: null,
            temperature: null,
            topP: null,
            additionalParameters: {},
          },
        ],
      },
    })),
  });
  vi.mocked(HarnessService.selectModel).mockResolvedValue({ ...session, model: nextModel });
  await projection.selectModel(nextModel);
  const nextResource = { kind: "doc" as const, id: "docs/next.yssbi-doc" };
  projection.addResource(nextResource);
  expect(projection.getState().selectedModel).toEqual(nextModel);
  vi.mocked(HarnessService.submitTurn).mockResolvedValueOnce({ finalText: "Done" });
  await projection.sendNextQueued();
  expect(HarnessService.submitTurn).toHaveBeenLastCalledWith(
    "session-1",
    "Follow up",
    [originalResource],
    testModelIdentity.selection,
  );
  expect(projection.getState().queuedMessages).toEqual([]);
  expect(projection.getState().draftResources).toEqual([nextResource]);
  projection.stop();
});

it("publishes replay as a complete projection and resumes cached sessions after their last event", async () => {
  vi.mocked(HarnessService.subscribeEvents).mockImplementationOnce(
    async (_session, after, callback) => {
      expect(after).toBe(0);
      callback(event(1));
      callback({
        ...event(2),
        turnId: "turn-1",
        type: "turn_started",
        payload: { model: testModelIdentity, resources: [], userMessage: "Long history" },
      });
      for (let sequence = 3; sequence < 1003; sequence++)
        callback({
          ...event(sequence),
          turnId: "turn-1",
          type: "text_delta",
          payload: { delta: "x" },
        });
      callback({
        ...event(1003),
        turnId: "turn-1",
        type: "turn_completed",
        payload: { finalText: "x".repeat(1000) },
      });
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  const projection = new AssistantHarnessProjection();
  const publishedLengths: number[] = [];
  const unsubscribe = projection.subscribe((state) =>
    publishedLengths.push(
      state.messages[1]?.content.reduce(
        (length, part) => length + (part.type === "text" ? part.text.length : 0),
        0,
      ) ?? 0,
    ),
  );
  await projection.start(session.sessionId);
  expect(publishedLengths.every((length) => length === 0 || length === 1000)).toBe(true);
  expect(projection.getState().lastSequence).toBe(1003);
  vi.mocked(HarnessService.openSession).mockResolvedValue(session);
  vi.mocked(HarnessService.subscribeEvents).mockImplementationOnce(async (_session, after) => {
    expect(after).toBe(1003);
    return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
  });
  await projection.start(session.sessionId);
  expect(projection.getState().messages[1].content).toEqual([
    { type: "text", text: "x".repeat(1000) },
  ]);
  unsubscribe();
  projection.stop();
});

it("advances a queued follow-up when its terminal event arrives after the submit response", async () => {
  let onEvent!: (event: HarnessEvent) => void;
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, _after, callback) => {
      onEvent = callback;
      callback(event(1));
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  vi.mocked(HarnessService.submitTurn)
    .mockImplementationOnce(async () => {
      onEvent({
        ...event(2),
        turnId: "turn-1",
        type: "turn_started",
        payload: { model: testModelIdentity, resources: [], userMessage: "Run" },
      });
      return { finalText: "Finished" };
    })
    .mockResolvedValue({ finalText: "Next" });
  const projection = new AssistantHarnessProjection();
  await projection.start(session.sessionId);
  projection.updateModels(testModelCatalog);
  const pending = projection.submit("Run");
  projection.queueMessage("Follow up");
  await pending;
  expect(HarnessService.submitTurn).toHaveBeenCalledTimes(1);
  onEvent({
    ...event(3),
    turnId: "turn-1",
    type: "turn_completed",
    payload: { finalText: "Finished" },
  });
  await vi.waitFor(() => expect(projection.getState().isSubmitting).toBe(false));
  expect(HarnessService.submitTurn).toHaveBeenCalledTimes(2);
  expect(HarnessService.submitTurn).toHaveBeenLastCalledWith(
    "session-1",
    "Follow up",
    [],
    testModelIdentity.selection,
  );
  expect(projection.getState().queuedMessages).toEqual([]);
  projection.stop();
});

it("keeps a running conversation intact when another session's submission fails after switching", async () => {
  const other = { ...session, sessionId: "session-2" };
  const histories = new Map<string, HarnessEvent[]>();
  const listeners = new Map<string, (event: HarnessEvent) => void>();
  const emit = (value: HarnessEvent) => {
    histories.set(value.sessionId, [...(histories.get(value.sessionId) ?? []), value]);
    listeners.get(value.sessionId)?.(value);
  };
  vi.mocked(HarnessService.openSession).mockImplementation(async (id) =>
    id === other.sessionId ? other : session,
  );
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (sessionId, after, callback) => {
      listeners.set(sessionId, callback);
      for (const value of histories.get(sessionId) ?? [])
        if (value.sequence > after) callback(value);
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  let failFirst!: () => void;
  let finishSecond!: () => void;
  vi.mocked(HarnessService.submitTurn).mockImplementation((sessionId, text) => {
    emit({
      ...event(1),
      sessionId,
      occurredAt: 2000,
      turnId: sessionId,
      type: "turn_started",
      payload: { model: testModelIdentity, resources: [], userMessage: text },
    });
    return new Promise((resolve, reject) => {
      if (sessionId === session.sessionId)
        failFirst = () => {
          emit({
            ...event(2),
            sessionId,
            occurredAt: 5000,
            turnId: sessionId,
            type: "turn_failed",
          });
          reject({ code: "assistant_turn_failed", details: {}, incidentId: null });
        };
      else
        finishSecond = () => {
          emit({
            ...event(2),
            sessionId,
            occurredAt: 9000,
            turnId: sessionId,
            type: "turn_completed",
            payload: { finalText: "Second completed" },
          });
          resolve({ finalText: "Second completed" });
        };
    });
  });
  const projection = new AssistantHarnessProjection();
  await projection.start(session.sessionId);
  projection.updateModels(testModelCatalog);
  const first = projection.submit("First");
  await projection.start(other.sessionId);
  const second = projection.submit("Second");
  failFirst();
  expect(await first).toBe(true);
  expect(projection.getState()).toMatchObject({
    sessionId: other.sessionId,
    error: null,
    isRunning: true,
    isSubmitting: true,
  });
  expect(projection.getState().messages[1]).toMatchObject({
    finishedAt: null,
    status: { type: "running" },
  });
  finishSecond();
  await second;
  expect(projection.getState().messages[1].finishedAt).toBe(9000);
  await projection.start(session.sessionId);
  expect(projection.getState()).toMatchObject({
    isRunning: false,
    isSubmitting: false,
    unsentMessage: null,
  });
  expect(projection.getState().messages[1]).toMatchObject({
    finishedAt: 5000,
    status: { type: "incomplete", reason: "error" },
  });
  projection.stop();
});

it("keeps sending disabled until event replay and subscription complete", async () => {
  let finishSubscription!: (
    value: Awaited<ReturnType<typeof HarnessService.subscribeEvents>>,
  ) => void;
  const subscription = new Promise<Awaited<ReturnType<typeof HarnessService.subscribeEvents>>>(
    (resolve) => {
      finishSubscription = resolve;
    },
  );
  let notifySubscribed!: () => void;
  const subscribed = new Promise<void>((resolve) => {
    notifySubscribed = resolve;
  });
  const unsubscribe = vi.fn().mockResolvedValue(undefined);
  vi.mocked(HarnessService.subscribeEvents).mockImplementationOnce((_session, _after, onEvent) => {
    onEvent(event(1));
    onEvent({
      ...event(2),
      turnId: "turn-1",
      type: "turn_started",
      payload: { model: testModelIdentity, resources: [], userMessage: "Restored prompt" },
    });
    onEvent({
      ...event(3),
      turnId: "turn-1",
      type: "turn_completed",
      payload: { finalText: "Restored reply" },
    });
    notifySubscribed();
    return subscription;
  });
  const projection = new AssistantHarnessProjection();
  try {
    const start = projection.start(session.sessionId);
    await subscribed;
    projection.updateModels(testModelCatalog);
    expect(projection.isSendDisabled()).toBe(true);
    expect(await projection.submit("Do not send during replay")).toBe(false);
    expect(HarnessService.submitTurn).not.toHaveBeenCalled();
    finishSubscription({ unsubscribe });
    await start;

    expect(projection.getState()).toMatchObject({ status: "ready", lastSequence: 3 });
    expect(projection.getState().messages).toHaveLength(2);
    expect(projection.isSendDisabled()).toBe(false);
  } finally {
    finishSubscription({ unsubscribe });
    projection.stop();
  }
});

it("rejects a discontinuous recovery before accepting its subscription", async () => {
  const unsubscribeInitial = vi.fn().mockResolvedValue(undefined);
  const unsubscribeRecovery = vi.fn().mockResolvedValue(undefined);
  let failStream!: Parameters<typeof HarnessService.subscribeEvents>[3];
  vi.mocked(HarnessService.subscribeEvents)
    .mockImplementationOnce(async (_session, _after, onEvent, onError) => {
      onEvent(event(1));
      failStream = onError;
      return { unsubscribe: unsubscribeInitial };
    })
    .mockImplementationOnce(async (_session, after, onEvent) => {
      expect(after).toBe(1);
      onEvent(event(3));
      return { unsubscribe: unsubscribeRecovery };
    });
  const projection = new AssistantHarnessProjection();
  try {
    await projection.start(session.sessionId);
    projection.updateModels(testModelCatalog);
    expect(projection.isSendDisabled()).toBe(false);
    failStream(new Error("channel failed"));
    await vi.waitFor(() => expect(projection.getState().status).not.toBe("initializing"));

    expect(projection.getState()).toMatchObject({ status: "error", lastSequence: 1 });
    expect(projection.isSendDisabled()).toBe(true);
    expect(unsubscribeInitial).toHaveBeenCalledOnce();
    expect(unsubscribeRecovery).toHaveBeenCalledOnce();
    expect(HarnessService.subscribeEvents).toHaveBeenCalledTimes(2);
  } finally {
    projection.stop();
  }
});

it("restores referenced drafts and queued targets in a new projection after a rejected submission", async () => {
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, _after, onEvent) => {
      onEvent(event(1));
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  vi.mocked(HarnessService.submitTurn).mockRejectedValue({
    code: "assistant_resource_unavailable",
    details: null,
    incidentId: null,
  });
  const original = new AssistantHarnessProjection();
  await original.start(session.sessionId);
  original.updateModels(testModelCatalog);
  const first = { kind: "doc" as const, id: "docs/first.yssbi-doc" };
  original.addResource(first);
  original.queueMessage("Queued review");
  first.id = "docs/changed-by-caller.yssbi-doc";
  const second = { kind: "database" as const, id: "survey" };
  original.addResource(second);
  expect(await original.submit("Review this dataset")).toBe(false);
  expect(original.getState().status).toBe("ready");
  original.stop();
  vi.mocked(HarnessService.openSession).mockResolvedValue(session);
  const reopened = new AssistantHarnessProjection();
  await reopened.start(session.sessionId);
  expect(reopened.getState()).toMatchObject({
    draftResources: [second],
    unsentMessage: {
      text: "Review this dataset",
      resources: [second],
      model: testModelIdentity.selection,
    },
    queuedMessages: [
      { text: "Queued review", resources: [{ kind: "doc", id: "docs/first.yssbi-doc" }] },
    ],
    queuePaused: true,
  });
  reopened.stop();
});
