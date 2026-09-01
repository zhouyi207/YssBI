// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChartContainerSize } from "./useChartContainerSize";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

class ControllableResizeObserver {
  readonly observe = vi.fn();
  readonly unobserve = vi.fn();
  readonly disconnect = vi.fn();

  constructor(private readonly callback: ResizeObserverCallback) {
    currentObserver = this;
  }

  queue(width: number, height: number): void {
    containerWidth = width;
    containerHeight = height;
    act(() => this.callback([], this as unknown as ResizeObserver));
  }

  fire(width: number, height: number): void {
    this.queue(width, height);
    flushNextFrame();
  }
}

let containerWidth = 640;
let containerHeight = 320;
let currentObserver: ControllableResizeObserver | null = null;
let nextFrameId = 0;
let pendingFrames = new Map<number, FrameRequestCallback>();

const requestFrame = vi.fn((callback: FrameRequestCallback): number => {
  const frameId = ++nextFrameId;
  pendingFrames.set(frameId, callback);
  return frameId;
});

const cancelFrame = vi.fn((frameId: number): void => {
  pendingFrames.delete(frameId);
});

function flushNextFrame(): void {
  const next = pendingFrames.entries().next();
  if (next.done) throw new Error("Expected a queued animation frame");
  const [frameId, callback] = next.value;
  pendingFrames.delete(frameId);
  act(() => callback(0));
}

function SizeProbe({ onRender }: { onRender: () => void }) {
  onRender();
  const { containerRef, size } = useChartContainerSize();
  return (
    <div ref={containerRef} data-testid="container">
      <span data-testid="probe">
        {size.width}x{size.height}
      </span>
    </div>
  );
}

describe("useChartContainerSize", () => {
  let host: HTMLDivElement;
  let root: Root;
  let mounted: boolean;

  beforeEach(() => {
    containerWidth = 640;
    containerHeight = 320;
    currentObserver = null;
    nextFrameId = 0;
    pendingFrames = new Map();
    requestFrame.mockClear();
    cancelFrame.mockClear();
    vi.stubGlobal("ResizeObserver", ControllableResizeObserver);
    vi.stubGlobal("requestAnimationFrame", requestFrame);
    vi.stubGlobal("cancelAnimationFrame", cancelFrame);
    vi.spyOn(HTMLElement.prototype, "clientWidth", "get").mockImplementation(() => containerWidth);
    vi.spyOn(HTMLElement.prototype, "clientHeight", "get").mockImplementation(
      () => containerHeight,
    );
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    mounted = false;
  });

  afterEach(() => {
    if (mounted) act(() => root.unmount());
    host.remove();
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("coalesces measurements, skips equal sizes, and releases observer resources", () => {
    let renderCount = 0;
    mounted = true;
    act(() => root.render(<SizeProbe onRender={() => renderCount++} />));

    const container = host.querySelector<HTMLElement>('[data-testid="container"]');
    const probe = host.querySelector<HTMLElement>('[data-testid="probe"]');
    const observer = currentObserver;
    if (!container || !probe || !observer) throw new Error("Probe did not mount");
    expect(observer.observe).toHaveBeenCalledOnce();
    expect(observer.observe).toHaveBeenCalledWith(container);

    flushNextFrame();
    expect(probe.textContent).toBe("640x320");
    observer.fire(800, 400);
    expect(probe.textContent).toBe("800x400");
    const rendersAfterResize = renderCount;
    observer.fire(800, 400);
    expect(renderCount).toBe(rendersAfterResize);

    requestFrame.mockClear();
    observer.queue(900, 450);
    observer.queue(960, 480);
    expect(requestFrame).toHaveBeenCalledOnce();
    flushNextFrame();
    expect(probe.textContent).toBe("960x480");

    observer.queue(1024, 512);
    const pendingFrameId = nextFrameId;
    act(() => root.unmount());
    mounted = false;
    expect(observer.disconnect).toHaveBeenCalledOnce();
    expect(cancelFrame).toHaveBeenCalledWith(pendingFrameId);
    expect(pendingFrames.size).toBe(0);
  });
});
