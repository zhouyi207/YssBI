import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { parseIpcErrorDto } from "@/shared/types/dto/ipcError";
import {
  IPC_MALFORMED_ERROR_CODE,
  IPC_TRANSPORT_FAILURE_CODE,
  IpcError,
  isIpcErrorCode,
} from "./ipcError";
import { invokeCommand } from "./invokeCommand";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const backendError = {
  code: "resource_revision_conflict",
  details: { resourcePath: "events/Main.yssbi-event" },
  incidentId: "incident-42",
};

describe("IPC error wire contract", () => {
  it("parses the exact backend error shape", () => {
    expect(parseIpcErrorDto(backendError)).toEqual(backendError);
  });

  it.each([
    ["missing details", { code: "internal_error", incidentId: null }],
    ["missing incidentId", { code: "internal_error", details: null }],
    [
      "legacy message",
      { code: "internal_error", details: null, incidentId: null, message: "legacy" },
    ],
    ["extra field", { code: "internal_error", details: null, incidentId: null, extra: true }],
    ["uppercase code", { code: "INTERNAL_ERROR", details: null, incidentId: null }],
    ["hyphenated code", { code: "internal-error", details: null, incidentId: null }],
    ["empty code segment", { code: "internal__error", details: null, incidentId: null }],
    ["array details", { code: "internal_error", details: [], incidentId: null }],
    ["numeric incidentId", { code: "internal_error", details: null, incidentId: 42 }],
  ])("rejects %s", (_label, value) => {
    expect(() => parseIpcErrorDto(value)).toThrow("Invalid IPC error response");
  });
});

describe("invokeCommand", () => {
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("forwards Channel-compatible args and invoke options without cloning", async () => {
    const channel = { onmessage: vi.fn() };
    const args = { projectInstanceId: "project-1", onEvent: channel };
    const options = { headers: { "x-request-id": "request-1" } };
    vi.mocked(invoke).mockResolvedValue(undefined);

    await expect(
      invokeCommand<void>("execute_compiled_graph", args, options),
    ).resolves.toBeUndefined();

    expect(invoke).toHaveBeenCalledWith("execute_compiled_graph", args, options);
    expect(vi.mocked(invoke).mock.calls[0]?.[1]).toBe(args);
  });

  it("normalizes a structured backend rejection", async () => {
    vi.mocked(invoke).mockRejectedValue(backendError);

    const caught = await invokeCommand("save_project_graph").catch((error: unknown) => error);

    expect(caught).toBeInstanceOf(IpcError);
    expect(caught).toMatchObject({
      kind: "backend",
      command: "save_project_graph",
      code: backendError.code,
      details: backendError.details,
      incidentId: backendError.incidentId,
      cause: backendError,
    });
    expect(isIpcErrorCode(caught, "resource_revision_conflict")).toBe(true);
    expect(isIpcErrorCode(caught, "stale_project_lifecycle")).toBe(false);
  });

  it("classifies native errors as transport failures", async () => {
    const transportError = new TypeError("window.__TAURI_INTERNALS__ is unavailable");
    vi.mocked(invoke).mockRejectedValue(transportError);

    await expect(invokeCommand("get_project_index")).rejects.toMatchObject({
      kind: "transport",
      code: IPC_TRANSPORT_FAILURE_CODE,
      details: null,
      incidentId: null,
      cause: transportError,
    });
  });

  it.each([
    ["legacy string rejection", "legacy backend error"],
    ["legacy message object", { code: "internal_error", message: "legacy backend error" }],
    [
      "almost structured rejection",
      { code: "internal_error", details: null, incidentId: null, extra: true },
    ],
  ])("classifies %s as a malformed error", async (_label, rejection) => {
    vi.mocked(invoke).mockRejectedValue(rejection);

    await expect(invokeCommand("get_project_index")).rejects.toMatchObject({
      kind: "malformed",
      code: IPC_MALFORMED_ERROR_CODE,
      details: null,
      incidentId: null,
      cause: rejection,
    });
  });

  it("does not duck-type arbitrary objects in isIpcErrorCode", () => {
    expect(isIpcErrorCode(backendError, backendError.code)).toBe(false);
  });
});
