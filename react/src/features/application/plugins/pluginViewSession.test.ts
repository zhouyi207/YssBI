import { beforeEach, expect, it, vi } from "vitest";
import { PluginViewSession } from "./pluginViewSession";
import { normalizeIpcError } from "@/services/ipc/ipcError";
import type { ViewSession } from "@/shared/types/plugins/generated";
const mock = vi.hoisted(() => ({ attach: vi.fn(), detach: vi.fn() }));
vi.mock("@/services/plugins/pluginService", () => ({ pluginService: mock }));
beforeEach(() => {
  mock.attach.mockReset();
  mock.detach.mockReset().mockResolvedValue(undefined);
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((next) => {
    resolve = next;
  });
  return { promise, resolve };
}
const session = (id: string): ViewSession => ({
  sessionId: id,
  html: "<p>verified</p>",
  installationGeneration: "1",
});

it("drains an obsolete pending attach and waits for release before its successor", async () => {
  const slot = new PluginViewSession();
  const started = deferred<void>();
  const attached = deferred<ViewSession>();
  const releasing = deferred<void>();
  const released = deferred<void>();
  mock.attach
    .mockImplementationOnce(() => {
      started.resolve();
      return attached.promise;
    })
    .mockResolvedValue(session("new"));
  mock.detach.mockImplementationOnce(() => {
    releasing.resolve();
    return released.promise;
  });
  const first = slot.open("example.plugin", "runtime");
  await started.promise;
  const second = slot.open("example.plugin", "analysis");
  attached.resolve(session("old"));
  await releasing.promise;
  expect(mock.attach).toHaveBeenCalledOnce();
  expect(mock.detach).toHaveBeenCalledWith("old");
  released.resolve();
  await expect(first).resolves.toBeNull();
  await expect(second).resolves.toEqual(session("new"));
  expect(mock.attach).toHaveBeenCalledTimes(2);
  await slot.close();
  expect(mock.detach).toHaveBeenLastCalledWith("new");
});

it("retains failed releases for retry and preserves backend failure categories", async () => {
  const slot = new PluginViewSession();
  mock.attach.mockResolvedValueOnce(session("old")).mockResolvedValue(session("new"));
  await slot.open("example.plugin", "runtime");
  const error = (code: string) =>
    normalizeIpcError("plugin_view", {
      code: "plugin_request_failed",
      details: { pluginCode: code },
      incidentId: null,
    });
  mock.detach.mockRejectedValueOnce(error("plugin_state_unavailable"));
  await expect(slot.open("example.plugin", "analysis")).rejects.toMatchObject({
    failure: { phase: "detach", code: "plugin_state_unavailable" },
  });
  expect(mock.attach).toHaveBeenCalledOnce();
  await expect(slot.open("example.plugin", "analysis")).resolves.toEqual(session("new"));
  expect(mock.detach.mock.calls).toEqual([["old"], ["old"]]);
  await slot.close();
  mock.attach.mockRejectedValueOnce(error("plugin_view_limit"));
  await expect(slot.open("example.plugin", "analysis")).rejects.toMatchObject({
    failure: { phase: "attach", code: "plugin_view_limit" },
  });
});
