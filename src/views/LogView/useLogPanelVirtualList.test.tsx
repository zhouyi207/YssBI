// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  measure: vi.fn(),
  snapToBottom: vi.fn(),
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: () => ({ measure: mocks.measure }),
}));

vi.mock("./logPanelViewport", () => ({
  snapLogViewportToBottom: mocks.snapToBottom,
}));

import { useLogPanelVirtualList } from "./useLogPanelVirtualList";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const EMPTY_LOGS = [] as const;

class ControllableResizeObserver {
  readonly observe = vi.fn();
  readonly unobserve = vi.fn();
  readonly disconnect = vi.fn();

  constructor(private readonly callback: ResizeObserverCallback) {
    currentObserver = this;
  }

  fire(): void {
    this.callback([], this as unknown as ResizeObserver);
  }
}

let currentObserver: ControllableResizeObserver | null = null;

function VirtualListProbe() {
  const { viewportRef } = useLogPanelVirtualList({
    filteredLogs: EMPTY_LOGS,
    autoScroll: true,
    presentation: "standalone",
    refreshScrollToken: 0,
  });

  return <div ref={viewportRef} data-testid="viewport" />;
}

describe("useLogPanelVirtualList", () => {
  let host: HTMLDivElement;
  let root: Root;
  let mounted: boolean;

  beforeEach(() => {
    currentObserver = null;
    mocks.measure.mockReset();
    mocks.snapToBottom.mockReset();
    vi.stubGlobal("ResizeObserver", ControllableResizeObserver);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    mounted = false;
  });

  afterEach(() => {
    if (mounted) act(() => root.unmount());
    host.remove();
    vi.unstubAllGlobals();
    vi.clearAllMocks();
  });

  it("remeasures and follows the pinned tail when a standalone viewport resizes", () => {
    mounted = true;
    act(() => root.render(<VirtualListProbe />));

    const viewport = host.querySelector<HTMLElement>('[data-testid="viewport"]');
    const observer = currentObserver;
    expect(viewport).not.toBeNull();
    expect(observer).not.toBeNull();
    expect(observer?.observe).toHaveBeenCalledWith(viewport);

    mocks.measure.mockClear();
    mocks.snapToBottom.mockClear();
    act(() => observer?.fire());

    expect(mocks.measure).toHaveBeenCalledOnce();
    expect(mocks.snapToBottom).toHaveBeenCalledOnce();
    expect(mocks.snapToBottom).toHaveBeenCalledWith(viewport, 0);

    act(() => root.unmount());
    mounted = false;
    expect(observer?.disconnect).toHaveBeenCalledOnce();
  });
});
