import { describe, expect, it } from "vitest";
import { InvalidHarnessPayloadError, parseHarnessRuntimeStatus } from "./harnessContract";

describe("Harness wire contract", () => {
  it("requires an explicit provider status", () => {
    expect(parseHarnessRuntimeStatus({ providerConfigured: true })).toEqual({
      providerConfigured: true,
    });
    expect(() => parseHarnessRuntimeStatus({})).toThrow(InvalidHarnessPayloadError);
  });
});
