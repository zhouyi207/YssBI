import { beforeEach, expect, it, vi } from "vitest";
import { pluginService } from "@/services/plugins/pluginService";
import { fulfillPluginUiRequest } from "./pluginActions";

const mocks = vi.hoisted(() => ({ invoke: vi.fn(), save: vi.fn(), reveal: vi.fn() }));
vi.mock("@/services/ipc", () => ({ invokeCommand: mocks.invoke }));
vi.mock("@/services/platform/pathDialog", () => ({
  openPathDialog: vi.fn(),
  savePathDialog: mocks.save,
}));
vi.mock("@/services/platform/opener", () => ({ revealPath: mocks.reveal }));
vi.mock("@/features/core/ui/ui", () => ({ ui: { confirm: vi.fn() } }));

beforeEach(() => {
  vi.resetAllMocks();
  mocks.reveal.mockResolvedValue({ ok: true });
});

it("executes only validated host actions from their corresponding request methods", async () => {
  const reveal = { hostUi: { kind: "reveal", path: "C:/artifact.csv" } };
  const save = { hostUi: { kind: "saveFile", options: { defaultPath: "result.csv" } } };
  const open = {
    openView: {
      pluginId: "example.plugin",
      view: {
        id: "analysis",
        entry: "analysis.html",
        title: "Analysis",
        location: "editor",
        scope: "project",
      },
    },
  };
  for (const value of [reveal, save, open]) {
    mocks.invoke.mockResolvedValueOnce(value);
    const reply = await pluginService.call("session", "views.get_state", null);
    await expect(fulfillPluginUiRequest("session", reply, () => true)).resolves.toBe(value);
    expect(reply).toEqual({ kind: "result", value });
  }
  expect(mocks.reveal).not.toHaveBeenCalled();
  expect(mocks.save).not.toHaveBeenCalled();

  mocks.invoke.mockResolvedValueOnce(save);
  await expect(pluginService.call("session", "system.reveal_artifact", null)).rejects.toThrow();
  mocks.invoke.mockResolvedValueOnce({
    hostUi: { kind: "saveFile", options: { defaultPath: 42 } },
  });
  await expect(pluginService.call("session", "system.save_file", null)).rejects.toThrow();
  mocks.invoke.mockResolvedValueOnce({ openView: { ...open.openView, view: { id: "analysis" } } });
  await expect(pluginService.call("session", "views.open", null)).rejects.toThrow();

  mocks.invoke.mockResolvedValueOnce(reveal);
  const revealReply = await pluginService.call("session", "system.reveal_artifact", null);
  await fulfillPluginUiRequest("session", revealReply, () => true);
  expect(mocks.reveal).toHaveBeenCalledExactlyOnceWith("C:/artifact.csv");
  mocks.invoke.mockResolvedValueOnce(open);
  const openReply = await pluginService.call("session", "views.open", { viewId: "analysis" });
  expect(openReply).toEqual({ kind: "openView", value: open });
  await expect(fulfillPluginUiRequest("session", openReply, () => true)).resolves.toEqual(open);
});

it("does not grant exports after a pending save dialog outlives its view", async () => {
  let current = true;
  let select!: (value: { ok: true; value: string }) => void;
  mocks.invoke.mockResolvedValueOnce({
    hostUi: { kind: "saveFile", options: { defaultPath: "private/path/result.csv" } },
  });
  const reply = await pluginService.call("session", "system.save_file", null);
  mocks.save.mockImplementationOnce(
    () =>
      new Promise((resolve) => {
        select = resolve;
      }),
  );
  const pending = fulfillPluginUiRequest("session", reply, () => current);
  expect(mocks.save).toHaveBeenCalledWith({
    defaultPath: "result.csv",
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  current = false;
  select({ ok: true, value: "C:/result.csv" });
  await pending;
  expect(mocks.invoke).toHaveBeenCalledTimes(1);

  current = true;
  mocks.save.mockResolvedValueOnce({ ok: true, value: "C:/result.csv" });
  mocks.invoke.mockResolvedValueOnce("grant");
  await expect(fulfillPluginUiRequest("session", reply, () => current)).resolves.toEqual({
    ok: true,
    value: "grant",
  });
  expect(mocks.invoke).toHaveBeenLastCalledWith("grant_plugin_export", {
    sessionId: "session",
    path: "C:/result.csv",
  });
});
