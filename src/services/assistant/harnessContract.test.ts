import { describe, expect, it } from "vitest";
import {
  InvalidHarnessPayloadError,
  parseHarnessRuntimeStatus,
  parseHarnessSessions,
  parseHarnessEvent,
} from "./harnessContract";

describe("Harness wire contract", () => {
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
