// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { currentAppWindow, type AppWindowHandle } from "@/services/platform/appWindow";
import type { PlatformOutcome } from "@/services/platform/platformTypes";
import { useCurrentWindowActions, type CurrentWindowActions } from "./useCurrentWindowActions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("@/services/platform/appWindow", () => ({
  currentAppWindow: vi.fn(),
}));

interface FakeWindow extends AppWindowHandle {
  readonly triggerResize: () => void;
  readonly setMaximized: (value: boolean) => void;
}

function makeWindow(initialMaximized: boolean): FakeWindow {
  let maximized = initialMaximized;
  let resizeListener: (() => void) | undefined;
  const success = <T,>(value: T): Promise<PlatformOutcome<T>> =>
    Promise.resolve({ ok: true, value } as const);
  const handle: FakeWindow = {
    label: "main",
    show: vi.fn(() => success(undefined)),
    setTitle: vi.fn((_title: string) => success(undefined)),
    minimize: vi.fn(() => success(undefined)),
    toggleMaximize: vi.fn(() => success(undefined)),
    isMaximized: vi.fn(() => success(maximized)),
    close: vi.fn(() => success(undefined)),
    setDecorations: vi.fn((_enabled: boolean) => success(undefined)),
    outerPosition: vi.fn(() => success({ x: 0, y: 0 })),
    innerSize: vi.fn(() => success({ width: 800, height: 600 })),
    scaleFactor: vi.fn(() => success(1)),
    onCloseRequested: vi.fn((_listener) => success(vi.fn())),
    onResized: vi.fn((listener: () => void) => {
      resizeListener = listener;
      return success(vi.fn());
    }),
    triggerResize: () => resizeListener?.(),
    setMaximized: (value) => {
      maximized = value;
    },
  };
  return handle;
}

function failure(operation: "minimizeWindow"): PlatformOutcome<void> {
  return {
    ok: false,
    failure: { operation, code: "operationFailed", incidentId: "incident-window-42" },
  };
}

describe("useCurrentWindowActions", () => {
  let host: HTMLDivElement;
  let root: Root;
  let actions!: CurrentWindowActions;

  function Harness() {
    actions = useCurrentWindowActions();
    return (
      <output data-maximized={String(actions.maximized)} data-issue={actions.issue?.code ?? ""} />
    );
  }

  async function flush() {
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it("refreshes maximize state only from the currently mounted window generation", async () => {
    const previous = makeWindow(false);
    const current = makeWindow(false);
    vi.mocked(currentAppWindow).mockReturnValueOnce(previous).mockReturnValueOnce(current);

    await act(async () => root.render(<Harness />));
    await flush();
    await act(async () => root.unmount());
    root = createRoot(host);
    await act(async () => root.render(<Harness />));
    await flush();

    current.setMaximized(true);
    await act(async () => {
      current.triggerResize();
      await Promise.resolve();
    });
    expect(host.querySelector("output")?.dataset.maximized).toBe("true");

    previous.setMaximized(false);
    await act(async () => {
      previous.triggerResize();
      await Promise.resolve();
    });
    expect(host.querySelector("output")?.dataset.maximized).toBe("true");
    expect(previous.isMaximized).toHaveBeenCalledOnce();
  });

  it("maps a typed platform failure to a safe issue without rejecting the action", async () => {
    const window = makeWindow(false);
    vi.mocked(window.minimize).mockResolvedValueOnce(failure("minimizeWindow"));
    vi.mocked(currentAppWindow).mockReturnValue(window);

    await act(async () => root.render(<Harness />));
    await flush();
    let outcome: Awaited<ReturnType<CurrentWindowActions["minimize"]>> | undefined;
    await act(async () => {
      outcome = await actions.minimize();
    });

    expect(outcome).toEqual({ status: "failed" });
    expect(actions.issue).toEqual({
      code: "window_action_failed",
      incidentId: "incident-window-42",
    });
    expect(host.querySelector("output")?.dataset.issue).toBe("window_action_failed");
  });
});
