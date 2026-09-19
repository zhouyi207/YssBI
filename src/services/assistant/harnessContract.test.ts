import { describe, expect, it } from "vitest";
import {
  InvalidHarnessPayloadError,
  parseHarnessRuntimeStatus,
  parseHarnessSessions,
} from "./harnessContract";

describe("Harness wire contract", () => {
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
