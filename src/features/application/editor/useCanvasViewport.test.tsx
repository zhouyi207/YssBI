// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import {
  editorViewportScope,
  getViewport,
  setViewportLive,
  resetLiveViewports,
  useViewportStore,
} from "@/features/core/viewport";
import { useCanvasViewport } from "./useCanvasViewport";

const persist = vi.hoisted(() => vi.fn());
vi.mock("@/features/core/viewport/persistGraphViewport", () => ({ persistGraphViewport: persist }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("shares one viewport with navigation commands, preserves zoom, and isolates physical groups", () => {
  const host = document.createElement("div");
  const root = createRoot(host);
  let first: ReturnType<typeof useCanvasViewport>;
  let second: ReturnType<typeof useCanvasViewport>;
  function Harness() {
    first = useCanvasViewport("first", "graph");
    second = useCanvasViewport("second", "graph");
    return null;
  }
  try {
    useViewportStore.getState().clear();
    resetLiveViewports();
    act(() => root.render(<Harness />));
    const scope = editorViewportScope("first", "graph");
    const untouched = second!.viewport;
    act(() => setViewportLive(scope, { x: 40, y: 60, scale: 2 }));
    expect(first!.viewport).toEqual({ x: 40, y: 60, scale: 2 });
    expect(second!.viewport).toEqual(untouched);
    act(() => first!.setViewport({ x: 80, y: 90, scale: 0.5 }));
    expect(getViewport(scope)).toEqual({ x: 80, y: 90, scale: 0.5 });
    expect(persist).not.toHaveBeenCalled();
    act(() => first!.commit());
    expect(persist).toHaveBeenCalledWith(scope);
  } finally {
    act(() => root.unmount());
    useViewportStore.getState().clear();
    resetLiveViewports();
  }
});
