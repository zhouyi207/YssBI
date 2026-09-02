import { describe, expect, it } from "vitest";

import {
  InvalidHarnessPayloadError,
  parseHarnessEvent,
  parseHarnessRuntimeStatus,
} from "./harnessContract";

describe("Harness wire contract", () => {
  it("parses ordered turn events without accepting malformed capability ids", () => {
    expect(
      parseHarnessEvent({
        sequence: 2,
        sessionId: "session-1",
        turnId: "turn-1",
        occurredAt: 1_000,
        type: "turn_started",
        payload: { userMessage: "Inspect the dataset" },
      }),
    ).toMatchObject({
      sequence: 2,
      type: "turn_started",
      payload: { userMessage: "Inspect the dataset" },
    });

    expect(() =>
      parseHarnessEvent({
        sequence: 3,
        sessionId: "session-1",
        turnId: "turn-1",
        occurredAt: 1_001,
        type: "tool_invocation_requested",
        payload: { capabilityId: "delete_everything" },
      }),
    ).toThrow(InvalidHarnessPayloadError);
  });

  it("requires an explicit provider status", () => {
    expect(parseHarnessRuntimeStatus({ providerConfigured: true })).toEqual({
      providerConfigured: true,
    });
    expect(() => parseHarnessRuntimeStatus({})).toThrow(InvalidHarnessPayloadError);
  });
});
