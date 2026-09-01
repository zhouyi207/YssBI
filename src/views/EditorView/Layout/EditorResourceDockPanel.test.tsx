// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { IDockviewPanelProps } from "dockview-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelParams } from "@/features/core/dockview";
import type { EditorPanelScope, EditorRendererRegistry } from "@/modules/workbench/public";
import { EditorResourceDockPanel } from "./EditorResourceDockPanel";

const rendererCalls = {
  event: vi.fn(),
  function: vi.fn(),
  chart: vi.fn(),
};

const rendererRegistry = {
  event: (scope: EditorPanelScope<"event">) => {
    rendererCalls.event(scope);
    return <div data-editor-kind="event" />;
  },
  function: (scope: EditorPanelScope<"function">) => {
    rendererCalls.function(scope);
    return <div data-editor-kind="function" />;
  },
  chart: (scope: EditorPanelScope<"chart">) => {
    rendererCalls.chart(scope);
    return <div data-editor-kind="chart" />;
  },
} satisfies EditorRendererRegistry;

function createPanelApi() {
  const visibilityListeners = new Set<() => void>();
  const groupListeners = new Set<() => void>();
  const api = {
    id: "panel-1",
    isVisible: false,
    group: { id: "group-a" },
    onDidVisibilityChange(listener: () => void) {
      visibilityListeners.add(listener);
      return { dispose: () => visibilityListeners.delete(listener) };
    },
    onDidGroupChange(listener: () => void) {
      groupListeners.add(listener);
      return { dispose: () => groupListeners.delete(listener) };
    },
  };

  return {
    api,
    setVisible(isVisible: boolean) {
      api.isVisible = isVisible;
      visibilityListeners.forEach((listener) => listener());
    },
    setGroupId(groupId: string) {
      api.group.id = groupId;
      groupListeners.forEach((listener) => listener());
    },
  };
}

function panelProps(
  api: ReturnType<typeof createPanelApi>["api"],
  resourceKind: "event" | "function" | "chart",
): IDockviewPanelProps<WorkbenchPanelParams> {
  return {
    api,
    params: {
      metadata: {
        role: "editor",
        resourceRef: `${resourceKind}/Main`,
        resourceKind,
        pinned: true,
      },
    },
  } as unknown as IDockviewPanelProps<WorkbenchPanelParams>;
}

describe("EditorResourceDockPanel", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("selects the registered editor and forwards the live Dockview panel scope", () => {
    const panel = createPanelApi();

    act(() => {
      root.render(
        <EditorResourceDockPanel
          {...panelProps(panel.api, "event")}
          rendererRegistry={rendererRegistry}
        />,
      );
    });
    expect(host.querySelector('[data-editor-kind="event"]')).not.toBeNull();
    expect(rendererCalls.event).toHaveBeenLastCalledWith({
      panelInstanceId: "panel-1",
      groupId: "group-a",
      resourceRef: "event/Main",
      resourceKind: "event",
      isVisible: false,
    });

    act(() => {
      panel.setVisible(true);
      panel.setGroupId("group-b");
    });
    expect(rendererCalls.event).toHaveBeenLastCalledWith(
      expect.objectContaining({ groupId: "group-b", isVisible: true }),
    );

    act(() => {
      root.render(
        <EditorResourceDockPanel
          {...panelProps(panel.api, "function")}
          rendererRegistry={rendererRegistry}
        />,
      );
    });
    expect(host.querySelector('[data-editor-kind="function"]')).not.toBeNull();

    act(() => {
      root.render(
        <EditorResourceDockPanel
          {...panelProps(panel.api, "chart")}
          rendererRegistry={rendererRegistry}
        />,
      );
    });
    expect(host.querySelector('[data-editor-kind="chart"]')).not.toBeNull();
  });
});
