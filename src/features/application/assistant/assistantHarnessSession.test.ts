import { beforeEach, expect, it, vi } from "vitest";
import { HarnessService, type HarnessEvent } from "@/services/assistant/harnessService";
import { AssistantHarnessProjection } from "./assistantHarnessSession";

vi.mock("@/features/application/editor/editorGroupContext", () => ({
  getActiveGraphContext: () => null,
}));
vi.mock("@/services/assistant/harnessService", () => ({
  HarnessService: {
    configureProvider: vi.fn(),
    createSession: vi.fn(),
    listSessions: vi.fn(),
    listMemory: vi.fn(),
    subscribeEvents: vi.fn(),
  },
}));

const session = {
  sessionId: "session-1",
  projectInstanceId: "project-1",
  projectSessionId: "project-session-1",
  title: "",
  lastOpenedAt: 0,
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
  vi.resetAllMocks();
  vi.mocked(HarnessService.configureProvider).mockResolvedValue({ providerConfigured: true });
  vi.mocked(HarnessService.listSessions).mockResolvedValue([]);
  vi.mocked(HarnessService.createSession).mockResolvedValue(session);
  vi.mocked(HarnessService.listMemory).mockResolvedValue([]);
});

it("keeps a replayed memory deletion after the initial memory snapshot arrives", async () => {
  let resolveMemory!: (records: Awaited<ReturnType<typeof HarnessService.listMemory>>) => void;
  const memory = new Promise<Awaited<ReturnType<typeof HarnessService.listMemory>>>((resolve) => {
    resolveMemory = resolve;
  });
  let notifyMemoryRequested!: () => void;
  const memoryRequested = new Promise<void>((resolve) => {
    notifyMemoryRequested = resolve;
  });
  vi.mocked(HarnessService.listMemory).mockImplementationOnce(() => {
    notifyMemoryRequested();
    return memory;
  });
  const unsubscribe = vi.fn().mockResolvedValue(undefined);
  vi.mocked(HarnessService.subscribeEvents).mockImplementationOnce(
    async (_session, _after, onEvent) => {
      onEvent(event(1));
      onEvent({ ...event(2), type: "memory_deleted", payload: { recordId: "memory-1" } });
      return { unsubscribe };
    },
  );
  const projection = new AssistantHarnessProjection();
  try {
    const start = projection.start();
    await memoryRequested;
    await projection.syncProvider("test", "https://example.test/v1", "test");
    expect(projection.isSendDisabled()).toBe(true);
    resolveMemory([
      {
        recordId: "memory-1",
        scope: "session",
        kind: "research_question",
        status: "active",
        value: { question: "Initial memory" },
        createdAt: 1000,
        updatedAt: 1000,
      },
    ]);
    await start;

    expect(projection.getState()).toMatchObject({
      status: "ready",
      memoryRecords: [],
      memoryCount: 0,
      lastSequence: 2,
    });
    expect(projection.isSendDisabled()).toBe(false);
  } finally {
    resolveMemory([]);
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
    await projection.start();
    await projection.syncProvider("test", "https://example.test/v1", "test");
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
