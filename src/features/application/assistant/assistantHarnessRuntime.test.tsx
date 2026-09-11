// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { useExternalStoreRuntime, type AppendMessage } from "@assistant-ui/react";
import { HarnessService } from "@/services/assistant/harnessService";
import { parseHarnessEvent } from "@/services/assistant/harnessContract";
import { normalizeIpcError } from "@/services/ipc";
import { useAssistantHarnessRuntime } from "./assistantHarnessRuntime";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
vi.mock("@assistant-ui/react", async (importOriginal) => {
  const actual = await importOriginal<typeof import("@assistant-ui/react")>();
  return { ...actual, useExternalStoreRuntime: vi.fn(actual.useExternalStoreRuntime) };
});
vi.mock("@/features/core/settings/read", () => {
  const settings = {
    isLoading: true,
    ai: { openAiModel: "test", openAiBaseUrl: "https://example.test/v1", openAiApiKey: "test" },
  };
  return {
    getSettingsSnapshot: () => settings,
    useSettingsRead: (select: (value: typeof settings) => unknown) => select(settings),
  };
});
vi.mock("@/services/assistant/harnessService", () => ({
  HarnessService: {
    runtimeStatus: vi.fn(),
    configureProvider: vi.fn(),
    createSession: vi.fn(),
    subscribeEvents: vi.fn(),
    submitTurn: vi.fn(),
    closeSession: vi.fn(),
  },
}));
vi.mock("./assistantGraphTools", () => ({
  assistantActiveGraphPath: () => null,
  subscribeAssistantGraphTools: async () => ({ close: async () => {} }),
}));

afterEach(() => vi.resetAllMocks());

it("retains the safe provider failure and accepts a subsequent successful turn", async () => {
  const host = document.createElement("div");
  document.body.append(host);
  const root = createRoot(host);
  let runtime!: ReturnType<typeof useAssistantHarnessRuntime>["runtime"];
  let emit!: Parameters<typeof HarnessService.subscribeEvents>[2];
  vi.mocked(HarnessService.runtimeStatus).mockResolvedValue({ providerConfigured: true });
  vi.mocked(HarnessService.createSession).mockResolvedValue({
    sessionId: "session-1",
    projectInstanceId: "project-1",
    projectSessionId: "project-session-1",
  });
  vi.mocked(HarnessService.closeSession).mockResolvedValue();
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, _sequence, onEvent) => {
      emit = onEvent;
      emit(
        parseHarnessEvent({
          sessionId: "session-1",
          sequence: 1,
          turnId: null,
          occurredAt: 1000,
          type: "session_created",
        }),
      );
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  const event = (sequence: number, turnId: string, type: string, payload?: unknown) =>
    emit(
      parseHarnessEvent({
        sessionId: "session-1",
        sequence,
        turnId,
        occurredAt: 1000,
        type,
        ...(payload ? { payload } : {}),
      }),
    );
  vi.mocked(HarnessService.submitTurn)
    .mockImplementationOnce(async () => {
      event(2, "turn-1", "turn_started", { userMessage: "First request" });
      const capabilityId = "inspect_dataset_profile";
      event(3, "turn-1", "tool_invocation_requested", { capabilityId });
      event(4, "turn-1", "tool_invocation_started", { capabilityId, invocationId: "tool-1" });
      event(5, "turn-1", "tool_invocation_started", { capabilityId, invocationId: "tool-2" });
      event(6, "turn-1", "tool_invocation_completed", { capabilityId, invocationId: "tool-2" });
      event(7, "turn-1", "tool_invocation_failed", {
        capabilityId,
        invocationId: "tool-1",
        failureCode: "deadline_elapsed",
      });
      event(8, "turn-1", "tool_invocation_started", { capabilityId, invocationId: "tool-3" });
      event(9, "turn-1", "turn_failed");
      throw normalizeIpcError("submit_harness_turn", {
        code: "assistant_provider_connection_failed",
        details: null,
        incidentId: null,
      });
    })
    .mockImplementationOnce(async () => {
      event(10, "turn-2", "turn_started", { userMessage: "Second request" });
      event(11, "turn-2", "turn_completed", { finalText: "Recovered" });
      return { finalText: "Recovered" };
    });
  function Harness() {
    const state = useAssistantHarnessRuntime();
    runtime = state.runtime;
    const { snapshot } = state;
    return createElement(
      "output",
      { "data-status": snapshot.status, "data-error": snapshot.error?.code ?? "" },
      snapshot.messages.map((message) => message.text).join("\n"),
    );
  }
  const adapter = () => {
    const calls = vi.mocked(useExternalStoreRuntime).mock.calls;
    return calls[calls.length - 1][0];
  };
  const message = (text: string): AppendMessage => ({
    role: "user",
    content: [{ type: "text", text }],
    createdAt: new Date(1000),
    metadata: { custom: {} },
    attachments: [],
    parentId: null,
    sourceId: null,
    runConfig: {},
  });
  try {
    await act(async () => root.render(createElement(Harness)));
    expect(adapter().isSendDisabled).toBe(false);
    await act(async () => {
      await adapter().onNew!(message("First request"));
    });
    expect(host.querySelector("output")?.dataset.error).toBe(
      "assistant_provider_connection_failed",
    );
    expect(adapter().isSendDisabled).toBe(false);
    expect(
      runtime.thread.getState().messages[1].content.filter((part) => part.type === "tool-call"),
    ).toMatchObject([
      { toolCallId: "tool-1", isError: true, result: { status: "timed-out" } },
      { toolCallId: "tool-2", isError: false, result: { status: "completed" } },
      { toolCallId: "tool-3", isError: true, result: { status: "interrupted" } },
    ]);
    expect(
      runtime.thread
        .getState()
        .messages.map((message) => ({ role: message.role, status: message.status })),
    ).toEqual([
      { role: "user", status: undefined },
      { role: "assistant", status: { type: "incomplete", reason: "error" } },
    ]);
    await act(async () => {
      await adapter().onNew!(message("Second request"));
    });
    expect(HarnessService.submitTurn).toHaveBeenCalledTimes(2);
    expect(host.querySelector("output")?.dataset.error).toBe("");
    expect(host.textContent).toContain("Recovered");
    expect(adapter().isSendDisabled).toBe(false);
    expect(
      runtime.thread
        .getState()
        .messages.map((message) => ({ role: message.role, status: message.status })),
    ).toEqual([
      { role: "user", status: undefined },
      { role: "assistant", status: { type: "incomplete", reason: "error" } },
      { role: "user", status: undefined },
      { role: "assistant", status: { type: "complete", reason: "stop" } },
    ]);
  } finally {
    await act(async () => root.unmount());
    host.remove();
  }
});

it("replays a missing tool failure and ignores callbacks from the replaced subscription", async () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  let runtime!: ReturnType<typeof useAssistantHarnessRuntime>["runtime"];
  let snapshot!: ReturnType<typeof useAssistantHarnessRuntime>["snapshot"];
  const callbacks: Array<{
    event: Parameters<typeof HarnessService.subscribeEvents>[2];
    error: Parameters<typeof HarnessService.subscribeEvents>[3];
  }> = [];
  const event = (sequence: number, type: string, payload?: unknown) =>
    parseHarnessEvent({
      sessionId: "session-1",
      sequence,
      turnId: sequence === 1 ? null : "turn-1",
      occurredAt: 1000,
      type,
      ...(payload ? { payload } : {}),
    });
  vi.mocked(HarnessService.runtimeStatus).mockResolvedValue({ providerConfigured: true });
  vi.mocked(HarnessService.createSession).mockResolvedValue({
    sessionId: "session-1",
    projectInstanceId: "project-1",
    projectSessionId: "project-session-1",
  });
  vi.mocked(HarnessService.closeSession).mockResolvedValue();
  vi.mocked(HarnessService.subscribeEvents).mockImplementation(
    async (_session, afterSequence, onEvent, onError) => {
      callbacks.push({ event: onEvent, error: onError });
      if (callbacks.length === 1) onEvent(event(1, "session_created"));
      else {
        expect(afterSequence).toBe(3);
        onEvent(
          event(4, "tool_invocation_failed", {
            invocationId: "tool-1",
            capabilityId: "inspect_dataset_profile",
            failureCode: "cancelled",
          }),
        );
        onEvent(event(5, "turn_cancelled"));
      }
      return { unsubscribe: vi.fn().mockResolvedValue(undefined) };
    },
  );
  function Harness() {
    ({ runtime, snapshot } = useAssistantHarnessRuntime());
    return null;
  }
  try {
    await act(async () => root.render(createElement(Harness)));
    await act(async () => {
      callbacks[0].event(event(2, "turn_started", { userMessage: "Profile" }));
      callbacks[0].event(
        event(3, "tool_invocation_started", {
          invocationId: "tool-1",
          capabilityId: "inspect_dataset_profile",
        }),
      );
      callbacks[0].event(event(5, "turn_cancelled"));
    });
    expect(snapshot).toMatchObject({ status: "ready", isRunning: false, lastSequence: 5 });
    expect(
      runtime.thread.getState().messages[1].content.filter((part) => part.type === "tool-call"),
    ).toMatchObject([{ toolCallId: "tool-1", isError: true, result: { status: "cancelled" } }]);
    await act(async () => {
      callbacks[0].event(event(6, "text_delta", { delta: "stale" }));
      callbacks[0].error(new Error("stale subscription"));
    });
    expect(snapshot.lastSequence).toBe(5);
    expect(HarnessService.subscribeEvents).toHaveBeenCalledTimes(2);
  } finally {
    await act(async () => root.unmount());
  }
});

it("closes a session whose creation finishes after the panel unmounts", async () => {
  const root = createRoot(document.createElement("div"));
  let resolve!: (session: Awaited<ReturnType<typeof HarnessService.createSession>>) => void;
  vi.mocked(HarnessService.runtimeStatus).mockResolvedValue({ providerConfigured: true });
  vi.mocked(HarnessService.createSession).mockImplementation(
    () =>
      new Promise((done) => {
        resolve = done;
      }),
  );
  vi.mocked(HarnessService.closeSession).mockResolvedValue();
  function Harness() {
    useAssistantHarnessRuntime();
    return null;
  }
  await act(async () => root.render(createElement(Harness)));
  await act(async () => root.unmount());
  await act(async () =>
    resolve({
      sessionId: "late-session",
      projectInstanceId: "project-1",
      projectSessionId: "project-session-1",
    }),
  );
  expect(HarnessService.closeSession).toHaveBeenCalledWith("late-session");
  expect(HarnessService.subscribeEvents).not.toHaveBeenCalled();
});
