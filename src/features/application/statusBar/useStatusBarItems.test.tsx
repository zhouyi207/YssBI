// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { editorViewportScope, setViewportLive, useViewportStore } from "@/features/core/viewport";
import {
  useEditorPaneStateStore,
  workbenchDockviewRead,
  type WorkbenchEditorPanelInfo,
} from "@/modules/workbench/public";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { useStatusBarItems } from "./useStatusBarItems";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: translate }),
}));

function translate(key: string, options?: { count: number }) {
  return options ? `${key}:${options.count}` : key;
}

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const renderItems = vi.fn();

function StatusItems() {
  const items = useStatusBarItems();
  renderItems();
  return items.right.map((item) => (
    <div key={item.id} data-item={item.id}>
      {item.content}
    </div>
  ));
}

describe("status bar subscriptions", () => {
  let root: Root;
  let activeEditor: WorkbenchEditorPanelInfo | undefined;
  let snapshot = { revision: 0, ready: true, hydrated: true };
  const listeners = new Set<() => void>();
  const frames = new Map<number, FrameRequestCallback>();
  let frameId = 0;

  function publishLayout() {
    snapshot = { ...snapshot, revision: snapshot.revision + 1 };
    listeners.forEach((listener) => listener());
  }

  function flushFrame() {
    const pending = [...frames.values()];
    frames.clear();
    pending.forEach((callback) => callback(0));
  }

  function text(id: string) {
    return document.querySelector(`[data-item="${id}"]`)!.textContent;
  }

  beforeEach(() => {
    startProjectLifecycle("status-test-project");
    activeEditor = {
      panelInstanceId: "editor",
      groupId: "main",
      component: "EditorResource",
      metadata: { role: "editor", resourceRef: "Main", resourceKind: "event" },
      active: true,
      location: { type: "grid" },
    };
    vi.spyOn(workbenchDockviewRead, "getActiveEditorPanel").mockImplementation(() => activeEditor);
    vi.spyOn(workbenchDockviewRead, "getSnapshot").mockImplementation(() => snapshot);
    vi.spyOn(workbenchDockviewRead, "subscribe").mockImplementation((listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    });
    vi.stubGlobal("requestAnimationFrame", (callback: FrameRequestCallback) => {
      frames.set(++frameId, callback);
      return frameId;
    });
    vi.stubGlobal("cancelAnimationFrame", (id: number) => {
      frames.delete(id);
    });
    const host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    const { projection } = makeEditorProjectionFixture({ graphPath: "Main" });
    expect(useGraphProjectionStore.getState().replaceProjection("Main", projection).applied).toBe(
      true,
    );
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    expect(listeners.size).toBe(0);
    expect(frames.size).toBe(0);
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
    useGraphProjectionStore.setState({ graphEntities: {} });
    useEditorPaneStateStore.getState().reset();
    useViewportStore.getState().clear();
    clearProjectLifecycle();
    document.body.replaceChildren();
  });

  it("counts the current projection without walking pins or rerendering for unrelated updates", async () => {
    const scanPins = vi.spyOn(useGraphProjectionStore.getState(), "getGraphNodePins");
    await act(async () => root.render(<StatusItems />));
    expect(text("node-count")).toBe("bottomBar.nodes:1");
    expect(text("connection-count")).toBe("bottomBar.links:1");
    await act(async () => useEditorPaneStateStore.getState().setSelectedNodeIds("editor", ["a"]));
    expect(text("selected-nodes")).toBe("bottomBar.selected:1");
    renderItems.mockClear();

    await act(async () => {
      publishLayout();
      useEditorPaneStateStore.getState().setSelectedNodeIds("editor", ["b"]);
      const { projection } = makeEditorProjectionFixture({ graphPath: "Background" });
      expect(
        useGraphProjectionStore.getState().replaceProjection("Background", projection).applied,
      ).toBe(true);
    });
    expect(renderItems).not.toHaveBeenCalled();
    expect(scanPins).not.toHaveBeenCalled();

    const first = makeEditorProjectionFixture({ graphPath: "Main" }).projection;
    const second = makeEditorProjectionFixture({
      graphPath: "Main",
      nodeId: "second",
      connectionId: "second-link",
    }).projection;
    await act(async () => {
      expect(
        useGraphProjectionStore.getState().replaceProjection("Main", {
          ...first,
          nodes: [...first.nodes, ...second.nodes],
          connections: [...first.connections, ...second.connections],
        }).applied,
      ).toBe(true);
    });
    expect(text("node-count")).toBe("bottomBar.nodes:2");
    expect(text("connection-count")).toBe("bottomBar.links:2");

    await act(async () => {
      activeEditor = {
        ...activeEditor!,
        metadata: { role: "editor", resourceRef: "Background", resourceKind: "event" },
      };
      publishLayout();
    });
    expect(text("node-count")).toBe("bottomBar.nodes:1");
    expect(text("connection-count")).toBe("bottomBar.links:1");
    await act(async () => {
      activeEditor = undefined;
      publishLayout();
    });
    expect(text("node-count")).toBe("bottomBar.nodes:0");
    expect(text("connection-count")).toBe("bottomBar.links:0");
    expect(text("selected-nodes")).toBe("bottomBar.selected:0");
  });

  it("coalesces viewport text to the latest frame and cancels work when the editor loses focus", async () => {
    await act(async () => root.render(<StatusItems />));
    await act(async () => flushFrame());
    const viewport = document.querySelector<HTMLSpanElement>('[data-item="viewport-status"] span')!;
    const writes = vi.spyOn(viewport, "textContent", "set");
    const scope = editorViewportScope("main", "Main");
    renderItems.mockClear();

    for (let step = 1; step <= 100; step += 1) {
      setViewportLive(scope, { x: step, y: -step, scale: 1.25 });
    }
    expect(frames.size).toBe(1);
    expect(writes).not.toHaveBeenCalled();
    await act(async () => flushFrame());
    expect(viewport.textContent).toBe("X 100 Y -100 125%");
    expect(writes).toHaveBeenCalledTimes(1);
    expect(renderItems).not.toHaveBeenCalled();

    setViewportLive(scope, { x: 100.1 });
    await act(async () => flushFrame());
    expect(writes).toHaveBeenCalledTimes(1);
    setViewportLive(scope, { x: 200 });
    await act(async () => {
      activeEditor = undefined;
      publishLayout();
    });
    expect(frames.size).toBe(0);
    expect(viewport.textContent).toBe("X 0 Y 0 100%");
    setViewportLive(scope, { x: 300 });
    expect(frames.size).toBe(0);
  });
});
