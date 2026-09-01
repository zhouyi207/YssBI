// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { currentAppWindow } from "@/services/platform/appWindow";
import type { AppWindowHandle } from "@/services/platform/appWindow";
import { useWorkbenchWindowGeometryPersistence } from "./useWorkbenchWindowGeometryPersistence";

vi.mock("@/services/platform/appWindow", () => ({
  currentAppWindow: vi.fn(),
}));

function Harness(): null {
  useWorkbenchWindowGeometryPersistence();
  return null;
}

describe("useWorkbenchWindowGeometryPersistence", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    localStorage.clear();
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it("persists restorable geometry when a secondary window closes maximized", async () => {
    let closeListener: (() => Promise<"allow">) | undefined;
    const unlisten = vi.fn();
    vi.mocked(currentAppWindow).mockReturnValue({
      label: "window-2",
      onCloseRequested: vi.fn(async (listener: () => Promise<"allow">) => {
        closeListener = listener;
        return { ok: true, value: unlisten };
      }),
      isMaximized: vi.fn(async () => ({ ok: true, value: true })),
    } as unknown as AppWindowHandle);

    await act(async () => {
      root.render(<Harness />);
    });
    await act(async () => {
      await closeListener?.();
    });

    expect(JSON.parse(localStorage.getItem("yssbi-secondary-window-window-2") ?? "null")).toEqual({
      width: 1000,
      height: 700,
      x: expect.any(Number),
      y: expect.any(Number),
      isMaximized: true,
    });
  });

  it("disposes a close listener that resolves after unmount", async () => {
    let resolveListener: ((outcome: { ok: true; value: () => void }) => void) | undefined;
    const unlisten = vi.fn();
    vi.mocked(currentAppWindow).mockReturnValue({
      label: "window-2",
      onCloseRequested: vi.fn(
        () =>
          new Promise<{ ok: true; value: () => void }>((resolve) => {
            resolveListener = resolve;
          }),
      ),
    } as unknown as AppWindowHandle);

    await act(async () => {
      root.render(<Harness />);
    });
    await act(async () => {
      root.unmount();
    });
    resolveListener?.({ ok: true, value: unlisten });
    await Promise.resolve();

    expect(unlisten).toHaveBeenCalledOnce();
    root = createRoot(host);
  });
});
