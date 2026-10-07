import { beforeEach, expect, it, vi } from "vitest";
import { pluginService } from "./pluginService";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@/services/ipc", () => ({ invokeCommand: invoke }));

beforeEach(() => invoke.mockReset());

it("validates export grants and garbage collection counts at the command boundary", async () => {
  invoke.mockResolvedValueOnce(42).mockResolvedValueOnce("");
  await expect(pluginService.grantExport("session", "C:/result.csv")).rejects.toThrow();
  await expect(pluginService.grantExport("session", "C:/result.csv")).rejects.toThrow();
  invoke.mockResolvedValueOnce("grant");
  await expect(pluginService.grantExport("session", "C:/result.csv")).resolves.toBe("grant");
  expect(invoke).toHaveBeenLastCalledWith("grant_plugin_export", {
    sessionId: "session",
    path: "C:/result.csv",
  });

  for (const count of ["1", -1, 0.5, Number.MAX_SAFE_INTEGER + 1]) {
    invoke.mockResolvedValueOnce(count);
    await expect(pluginService.collectGarbage()).rejects.toThrow();
  }
  invoke.mockResolvedValueOnce(0).mockResolvedValueOnce(3);
  await expect(pluginService.collectGarbage()).resolves.toBe(0);
  await expect(pluginService.collectGarbage()).resolves.toBe(3);
});

it("accepts only generated projection fields and the requested plugin identity", async () => {
  const storage = {
    pluginId: "example.plugin",
    usedBytes: 8,
    cacheBytes: 2,
    budgetBytes: 100,
    enforcement: "soft",
  };
  invoke.mockResolvedValueOnce({ ...storage, constructor: "undeclared" });
  await expect(pluginService.storage("example.plugin")).rejects.toThrow();
  invoke.mockResolvedValueOnce({ ...storage, pluginId: "other.plugin" });
  await expect(pluginService.storage("example.plugin")).rejects.toThrow();
  invoke.mockResolvedValueOnce({ ...storage, pluginId: "other.plugin" });
  await expect(pluginService.clearCache("example.plugin")).rejects.toThrow();

  const task = {
    pluginId: "example.plugin",
    taskId: "task",
    operationId: "operation",
    packageDigest: "digest",
    revision: "1",
    state: "succeeded",
  };
  invoke.mockResolvedValueOnce({ tasks: [{ ...task, pluginId: "other.plugin" }] });
  await expect(pluginService.history("example.plugin")).rejects.toThrow();
  const diagnostic = {
    pluginId: "example.plugin",
    instanceId: "instance",
    stderr: "",
    taskIds: [],
    truncated: false,
  };
  invoke.mockResolvedValueOnce([{ ...diagnostic, pluginId: "other.plugin" }]);
  await expect(pluginService.diagnostics("example.plugin")).rejects.toThrow();

  invoke
    .mockResolvedValueOnce(storage)
    .mockResolvedValueOnce(storage)
    .mockResolvedValueOnce({ tasks: [task], nextCursor: "cursor" })
    .mockResolvedValueOnce([diagnostic]);
  await expect(pluginService.storage("example.plugin")).resolves.toBe(storage);
  await expect(pluginService.clearCache("example.plugin")).resolves.toBe(storage);
  await expect(pluginService.history("example.plugin")).resolves.toEqual({
    tasks: [task],
    nextCursor: "cursor",
  });
  await expect(pluginService.diagnostics("example.plugin")).resolves.toEqual([diagnostic]);
});
