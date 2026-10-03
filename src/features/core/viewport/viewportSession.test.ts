// @vitest-environment happy-dom
import { afterEach, expect, it, vi } from "vitest";
import { DEFAULT_VIEWPORT } from "@/shared/config-default";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { readLiveViewport, resetLiveViewports } from "./liveViewportState";
import { releaseEditorViewport, releaseGraphViewport, useViewportStore } from "./useViewportStore";
import { editorViewportScope, viewportScopeKey } from "./viewportScope";
import {
  commitViewport,
  getViewport,
  setViewportLive,
  subscribeToViewport,
} from "./viewportSession";

afterEach(() => {
  useViewportStore.getState().clear();
  clearProjectLifecycle();
});

it("preserves viewports written by committed-release subscribers", () => {
  const scope = editorViewportScope("pane", "events/graph");
  const opened = editorViewportScope("new-pane", "events/graph");
  const successor = { x: 80, y: 90, scale: 2 };
  for (const release of [
    () => useViewportStore.getState().clear(),
    () => releaseEditorViewport(scope),
    () => releaseGraphViewport(scope.graphPath),
  ]) {
    useViewportStore.getState().clear();
    setViewportLive(scope, { x: 10, y: 20, scale: 1 });
    commitViewport(scope);
    const unsubscribe = useViewportStore.subscribe(() => {
      setViewportLive(scope, successor);
      setViewportLive(opened, successor);
    });
    try {
      release();
      expect.soft(getViewport(scope)).toEqual(successor);
      expect.soft(getViewport(opened)).toEqual(successor);
    } finally {
      unsubscribe();
    }
  }
});

it("does not extend a live reset or snapshot synchronization into a successor publication", () => {
  const first = editorViewportScope("first", "events/graph");
  const second = editorViewportScope("second", "events/graph");
  const opened = editorViewportScope("opened", "events/graph");
  for (const mode of ["reset", "snapshot", "project"]) {
    // A new project's gesture can keep equal coordinates and therefore the same stored reference.
    const successor = mode === "project" ? { x: 10, y: 20, scale: 1 } : { x: 80, y: 90, scale: 2 };
    useViewportStore.getState().clear();
    for (const scope of [first, second]) {
      setViewportLive(scope, { x: 10, y: 20, scale: 1 });
      commitViewport(scope);
    }
    let armed = false;
    const unsubscribe = subscribeToViewport(first, () => {
      if (!armed) return;
      armed = false;
      if (mode === "snapshot")
        useViewportStore.setState({
          viewports: {
            [viewportScopeKey(first)]: successor,
            [viewportScopeKey(second)]: successor,
          },
        });
      else {
        if (mode === "project") startProjectLifecycle("successor");
        setViewportLive(second, successor);
      }
      setViewportLive(opened, successor);
    });
    armed = true;
    try {
      if (mode !== "snapshot") resetLiveViewports();
      else
        useViewportStore.setState({
          viewports: {
            [viewportScopeKey(first)]: { x: 30, y: 40, scale: 1 },
            [viewportScopeKey(second)]: { x: 30, y: 40, scale: 1 },
          },
        });
      expect.soft(armed).toBe(false);
      expect.soft(getViewport(second)).toEqual(successor);
      expect.soft(readLiveViewport(second)).toEqual(successor);
      expect.soft(getViewport(opened)).toEqual(successor);
    } finally {
      unsubscribe();
    }
  }
});

it("publishes live coordinates only to their pane and commits without a duplicate notification", () => {
  const left = editorViewportScope("left", "events/graph");
  const right = editorViewportScope("right", "events/graph");
  const first = vi.fn();
  const second = vi.fn();
  const other = vi.fn();
  const unsubscribers = [
    subscribeToViewport(left, first),
    subscribeToViewport(left, second),
    subscribeToViewport(right, other),
  ];
  try {
    first.mockClear();
    second.mockClear();
    other.mockClear();
    const viewport = { x: 30, y: 40, scale: 2 };
    setViewportLive(left, viewport);
    expect(first).toHaveBeenCalledExactlyOnceWith(viewport);
    expect(second).toHaveBeenCalledExactlyOnceWith(viewport);
    expect(other).not.toHaveBeenCalled();
    expect(getViewport(right)).toEqual(DEFAULT_VIEWPORT);
    expect(useViewportStore.getState().viewports[viewportScopeKey(left)]).toBeUndefined();
    commitViewport(left);
    expect(useViewportStore.getState().viewports[viewportScopeKey(left)]).toEqual(viewport);
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
    useViewportStore.getState().setViewport(left, { x: 60 });
    expect(first).toHaveBeenCalledTimes(2);
    expect(second).toHaveBeenCalledTimes(2);
  } finally {
    unsubscribers.forEach((unsubscribe) => unsubscribe());
  }
});

it("releases pane coordinates without losing active subscriptions or restoring a closed pane", () => {
  const scope = editorViewportScope("pane", "events/graph");
  const listener = vi.fn();
  const unsubscribe = subscribeToViewport(scope, listener);
  try {
    setViewportLive(scope, { x: 10, y: 20, scale: 2 });
    commitViewport(scope);
    releaseEditorViewport(scope);
    expect(getViewport(scope)).toEqual(DEFAULT_VIEWPORT);
    expect(readLiveViewport(scope)).toBeUndefined();
    listener.mockClear();
    setViewportLive(scope, { x: 5, y: 6, scale: 1 });
    expect(listener).toHaveBeenCalledExactlyOnceWith({ x: 5, y: 6, scale: 1 });
    unsubscribe();
    unsubscribe();
    releaseEditorViewport(scope);
    expect(readLiveViewport(scope)).toBeUndefined();
    listener.mockClear();
    setViewportLive(scope, { x: 15, y: 16, scale: 1 });
    expect(listener).not.toHaveBeenCalled();
  } finally {
    unsubscribe();
  }
});

it("reconciles committed snapshots without depending on a mounted viewport subscriber", () => {
  const scope = editorViewportScope("pane", "events/graph");
  setViewportLive(scope, { x: 10, y: 20, scale: 2 });
  commitViewport(scope);
  const restored = { x: 40, y: 50, scale: 3 };
  useViewportStore.setState({ viewports: { [viewportScopeKey(scope)]: restored } });
  expect(getViewport(scope)).toEqual(restored);
  useViewportStore.setState({ viewports: {} });
  expect(getViewport(scope)).toEqual(DEFAULT_VIEWPORT);
  expect(readLiveViewport(scope)).toBeUndefined();
});

it("releases uncommitted pane, graph and project viewports and notifies only their subscribers", () => {
  const left = editorViewportScope("left", "events/graph");
  const right = editorViewportScope("right", "events/graph");
  const detached = editorViewportScope("detached", "events/graph");
  const other = editorViewportScope("left", "events/other");
  const first = vi.fn();
  const second = vi.fn();
  const rightListener = vi.fn();
  const otherListener = vi.fn();
  const unsubscribers = [
    subscribeToViewport(left, first),
    subscribeToViewport(left, second),
    subscribeToViewport(right, rightListener),
    subscribeToViewport(other, otherListener),
  ];
  const viewport = { x: 10, y: 20, scale: 2 };
  try {
    for (const scope of [left, right, detached, other]) setViewportLive(scope, viewport);
    for (const listener of [first, second, rightListener, otherListener]) listener.mockClear();
    releaseEditorViewport(left);
    expect(first).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    expect(second).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    expect(rightListener).not.toHaveBeenCalled();
    expect(otherListener).not.toHaveBeenCalled();

    setViewportLive(left, viewport);
    first.mockClear();
    second.mockClear();
    releaseGraphViewport(left.graphPath);
    expect(first).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    expect(second).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    expect(rightListener).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    for (const scope of [left, right, detached]) {
      expect(getViewport(scope)).toEqual(DEFAULT_VIEWPORT);
      expect(readLiveViewport(scope)).toBeUndefined();
    }
    expect(getViewport(other)).toEqual(viewport);
    expect(otherListener).not.toHaveBeenCalled();

    useViewportStore.getState().clear();
    expect(otherListener).toHaveBeenCalledExactlyOnceWith(DEFAULT_VIEWPORT);
    expect(first).toHaveBeenCalledTimes(1);
    expect(second).toHaveBeenCalledTimes(1);
    expect(rightListener).toHaveBeenCalledTimes(1);
    expect(getViewport(other)).toEqual(DEFAULT_VIEWPORT);
  } finally {
    unsubscribers.forEach((unsubscribe) => unsubscribe());
  }
});
