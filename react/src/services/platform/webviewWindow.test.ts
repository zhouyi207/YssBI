import { beforeEach, describe, expect, it, vi } from "vitest";
import { createWebviewWindow } from "./webviewWindow";

const mocks = vi.hoisted(() => ({
  callbacks: new Map<string, () => void>(),
  unlisten: [vi.fn(), vi.fn()],
  resolveListeners: [] as Array<() => void>,
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  WebviewWindow: class {
    once(event: string, callback: () => void) {
      mocks.callbacks.set(event, callback);
      const index = mocks.resolveListeners.length;
      return new Promise<() => void>((resolve) => {
        mocks.resolveListeners.push(() => resolve(mocks.unlisten[index]!));
      });
    }
  },
}));

const request = {
  label: "logs-test",
  url: "index.html#/logs",
  title: "Logs",
  width: 1000,
  height: 600,
};

describe("native window creation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocks.callbacks.clear();
    mocks.resolveListeners.length = 0;
  });

  it("waits for native creation and disposes both listeners", async () => {
    const settled = vi.fn();
    const pending = createWebviewWindow(request).then(settled);
    mocks.resolveListeners.forEach((resolve) => resolve());
    await Promise.resolve();
    expect(settled).not.toHaveBeenCalled();

    mocks.callbacks.get("tauri://created")?.();
    await pending;
    expect(settled).toHaveBeenCalledWith({ ok: true, value: undefined });
    for (const unlisten of mocks.unlisten) expect(unlisten).toHaveBeenCalledOnce();
  });

  it("propagates asynchronous failure and cleans subscriptions that resolve afterwards", async () => {
    const pending = createWebviewWindow(request);
    mocks.callbacks.get("tauri://error")?.();
    expect(await pending).toEqual({
      ok: false,
      failure: { operation: "createWebviewWindow", code: "operationFailed" },
    });

    mocks.resolveListeners.forEach((resolve) => resolve());
    await Promise.resolve();
    for (const unlisten of mocks.unlisten) expect(unlisten).toHaveBeenCalledOnce();
  });
});
