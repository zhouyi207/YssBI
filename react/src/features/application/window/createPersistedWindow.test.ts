import { beforeEach, describe, expect, it, vi } from "vitest";
import { createWebviewWindow } from "@/services/platform/webviewWindow";
import { createPersistedWindow } from "./createPersistedWindow";

vi.mock("@/services/platform/webviewWindow", () => ({
  createWebviewWindow: vi.fn(),
}));

vi.mock("./windowDecorationPolicy", () => ({
  readWindowDecorationsFromSettings: () => false,
}));

describe("createPersistedWindow", () => {
  beforeEach(() => {
    vi.mocked(createWebviewWindow).mockReset();
    vi.mocked(createWebviewWindow).mockResolvedValue({ ok: true, value: undefined });
  });

  it("creates hidden with logical defaults and current decorations for native restoration", async () => {
    await createPersistedWindow({
      kind: "logs",
      label: "logs-2",
      url: "index.html#/logs",
      title: "Logs",
    });

    expect(createWebviewWindow).toHaveBeenCalledWith({
      label: "logs-2",
      url: "index.html#/logs",
      title: "Logs",
      width: 1000,
      height: 600,
      decorations: false,
      visible: false,
    });
  });

  it("exposes only the stable platform failure code to callers", async () => {
    vi.mocked(createWebviewWindow).mockResolvedValueOnce({
      ok: false,
      failure: { operation: "createWebviewWindow", code: "operationFailed" },
    });

    await expect(
      createPersistedWindow({
        kind: "plot",
        label: "window-3",
        url: "index.html#/editor",
        title: "YssBI Node Editor",
      }),
    ).rejects.toThrow("operationFailed");
  });
});
