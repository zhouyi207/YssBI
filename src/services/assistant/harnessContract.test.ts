import { describe, expect, it } from "vitest";
import {
  InvalidHarnessPayloadError,
  parseHarnessRuntimeStatus,
  parseHarnessMemoryRecord,
  parseHarnessSessions,
  parseHarnessEvent,
  parseHarnessTurnResult,
} from "./harnessContract";

describe("Harness wire contract", () => {
  it("accepts empty successful turn text in both the receipt and completion event", () => {
    for (const finalText of ["", "Completed"]) {
      const result = { finalText };
      expect(parseHarnessTurnResult(result)).toEqual(result);
      const event = {
        sequence: 1,
        sessionId: "session",
        turnId: "turn",
        occurredAt: 1,
        type: "turn_completed",
        payload: result,
      };
      expect(parseHarnessEvent(event)).toEqual(event);
    }
    for (const finalText of [undefined, null, 0, false, {}]) {
      expect(() => parseHarnessTurnResult({ finalText })).toThrow(InvalidHarnessPayloadError);
    }
  });
  it("parses memory lifecycle records and rejects unknown states", () => {
    const memory = {
      recordId: "memory-1",
      scope: "session",
      kind: "research_question",
      value: { type: "research_question", question: "How does education relate to income?" },
      createdAt: 1,
      updatedAt: 2,
    };
    for (const status of ["proposed", "active", "superseded", "invalidated", "deleted"]) {
      const record = { ...memory, status };
      expect(parseHarnessMemoryRecord(record)).toEqual(record);
    }
    expect(() => parseHarnessMemoryRecord({ ...memory, status: "unknown" })).toThrow(
      InvalidHarnessPayloadError,
    );
  });

  it("parses worker lifecycle and only permits model events inside worker output", () => {
    const base = { sequence: 1, sessionId: "session", turnId: "turn", occurredAt: 1 };
    const started = {
      ...base,
      type: "agent_run_started",
      payload: {
        runId: "worker",
        parentRunId: "manager",
        role: "review",
        objective: "Check the report",
      },
    };
    expect(parseHarnessEvent(started)).toEqual(started);
    const output = {
      ...base,
      type: "agent_run_output",
      payload: {
        runId: "worker",
        event: { type: "text_delta", payload: { delta: "worker progress" } },
      },
    };
    expect(parseHarnessEvent(output).type).toBe("agent_run_output");
    expect(() =>
      parseHarnessEvent({
        ...output,
        payload: {
          ...output.payload,
          event: { type: "turn_completed", payload: { finalText: "false completion" } },
        },
      }),
    ).toThrow(InvalidHarnessPayloadError);
    expect(() =>
      parseHarnessEvent({ ...started, payload: { ...started.payload, role: "unknown" } }),
    ).toThrow(InvalidHarnessPayloadError);
    const finished = {
      ...base,
      type: "agent_run_finished",
      payload: {
        runId: "worker",
        role: "review",
        state: "blocked",
        summary: "Missing results",
        blockedReason: "Evidence unavailable",
        warnings: [],
        evidenceCount: 0,
      },
    };
    expect(parseHarnessEvent(finished)).toEqual(finished);
    expect(() =>
      parseHarnessEvent({ ...finished, payload: { ...finished.payload, state: "running" } }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("accepts resource edit lifecycle events without accepting unknown capabilities", () => {
    const event = {
      sequence: 1,
      sessionId: "session",
      turnId: "turn",
      occurredAt: 1,
      type: "tool_invocation_completed",
      payload: { invocationId: "invocation", capabilityId: "edit_resource" },
    };
    expect(parseHarnessEvent(event)).toEqual(event);
    expect(() =>
      parseHarnessEvent({
        ...event,
        payload: { ...event.payload, capabilityId: "execute_arbitrary_code" },
      }),
    ).toThrow(InvalidHarnessPayloadError);
  });
  it("requires an explicit provider status", () => {
    expect(parseHarnessRuntimeStatus({ providerConfigured: true })).toEqual({
      providerConfigured: true,
    });
    expect(() => parseHarnessRuntimeStatus({})).toThrow(InvalidHarnessPayloadError);
  });
  it("preserves conversation identity and rejects malformed list metadata", () => {
    const sessions = [
      {
        sessionId: "first",
        projectInstanceId: "prior-runtime",
        projectSessionId: "old-session",
        title: "Previous analysis",
        lastOpenedAt: 2,
      },
      {
        sessionId: "second",
        projectInstanceId: "current-runtime",
        projectSessionId: "current-session",
        title: "",
        lastOpenedAt: 1,
      },
    ];
    expect(parseHarnessSessions(sessions)).toEqual(sessions);
    expect(() => parseHarnessSessions([sessions[0], sessions[0]])).toThrow(
      InvalidHarnessPayloadError,
    );
    expect(() => parseHarnessSessions([{ ...sessions[0], lastOpenedAt: -1 }])).toThrow(
      InvalidHarnessPayloadError,
    );
    expect(() => parseHarnessSessions([{ sessionId: "missing-metadata" }])).toThrow(
      InvalidHarnessPayloadError,
    );
  });
});
