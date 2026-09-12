// @vitest-environment happy-dom

import { act, Profiler } from "react";
import { createRoot, type Root } from "react-dom/client";
import { DockviewReact, type DockviewApi } from "dockview-react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import { useIsActiveEditorPanel } from "@/features/application/editor/useIsActiveEditorPanel";
import { workbenchDockviewInternal } from "../../dockview/workbenchDockviewInternal";
import { resetWorkbenchLayout } from "../../application/workbenchLayoutActions";
import { bindWorkbenchStatusBarLayout } from "../../application/workbenchStatusBarLayout";
import type { WorkbenchPanelParams } from "../../dockview/workbenchPanelModel";
import { StatusBar } from "./StatusBar";
import { workbenchUi } from "../../state/ui";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("i18next", () => ({ default: { t: (key: string) => key } }));

vi.mock("../../application/workbenchLayoutController", () => ({
  workbenchLayoutController: {
    beginLayoutReset: vi.fn(() => 1),
    completeLayoutReset: vi.fn(),
  },
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function Panel() {
  return null;
}

const editorRender = vi.fn();
const statusRender = vi.fn();

function EditorPanel() {
  editorRender(useIsActiveEditorPanel("editor"));
  return null;
}

describe("Workbench status panel tabs", () => {
  let root: Root | undefined;
  let api: DockviewApi;
  let unbindLayout: (() => void) | undefined;

  afterEach(async () => {
    await act(async () => {
      unbindLayout?.();
      workbenchDockviewInternal.unbind(api);
      root?.unmount();
      workbenchUi.setSettingsOpen(false);
    });
    vi.restoreAllMocks();
    document.body.replaceChildren();
  });

  async function update(action: () => void | Promise<unknown>) {
    await act(async () => {
      await action();
      await new Promise((resolve) => setTimeout(resolve, 30));
    });
  }

  async function renderWorkbench(collapsed = false) {
    const host = document.createElement("div");
    host.dataset.yssbiWorkbench = "";
    document.body.appendChild(host);
    root = createRoot(host);
    await update(() => {
      root!.render(
        <TooltipProvider>
          <div data-yssbi-root-dockview style={{ width: 900, height: 600 }}>
            <DockviewReact
              components={{
                EditorResource: EditorPanel,
                Problems: Panel,
                Output: Panel,
                Logs: Panel,
                Details: Panel,
                Assistant: Panel,
                Project: Panel,
                Nodes: Panel,
                Commands: Panel,
                Plugins: Panel,
              }}
              onReady={({ api: readyApi }) => {
                api = readyApi;
                const editor = api.addPanel<WorkbenchPanelParams>({
                  id: "editor",
                  component: "EditorResource",
                  title: "Main",
                  params: {
                    metadata: { role: "editor", resourceRef: "Main", resourceKind: "event" },
                  },
                });
                api.addEdgeGroup("bottom", { id: "bottom", initialSize: 200, collapsed });
                for (const [viewId, component] of [
                  ["problems", "Problems"],
                  ["output", "Output"],
                  ["logs", "Logs"],
                ] as const) {
                  api.addPanel<WorkbenchPanelParams>({
                    id: viewId,
                    component,
                    title: component,
                    params: { metadata: { role: "view", viewId } },
                    position: { referenceGroup: "bottom", direction: "within" },
                    inactive: true,
                  });
                }
                if (!collapsed) api.getPanel("problems")!.api.setActive();
                editor.api.setActive();
                workbenchDockviewInternal.bind(api);
                workbenchDockviewInternal.completeHydration();
                unbindLayout = bindWorkbenchStatusBarLayout(
                  api,
                  host.querySelector("[data-yssbi-root-dockview]")!,
                );
              }}
            />
          </div>
          <Profiler id="status" onRender={statusRender}>
            <StatusBar ariaLabel="status" left={[]} right={[]} />
          </Profiler>
        </TooltipProvider>,
      );
    });
  }

  function button(viewId: string): HTMLButtonElement {
    return document.querySelector(`footer [data-workbench-status-panel="${viewId}"]`)!;
  }

  it("hides all bottom views after reset and opens only the requested view afterward", async () => {
    await renderWorkbench();
    const views = ["problems", "output", "logs"].map((viewId) => api.getPanel(viewId)!);
    const editorGroup = api.getPanel("editor")!.group;
    await update(() => api.getPanel("logs")!.api.moveTo({ group: editorGroup }));

    await update(() => resetWorkbenchLayout());

    expect(api.getEdgeGroup("bottom")!.isCollapsed()).toBe(true);
    expect(api.isEdgeGroupVisible("bottom")).toBe(false);
    expect(api.activePanel?.id).toBe("editor");
    for (const panel of views) {
      expect(api.getPanel(panel.id)).toBe(panel);
      expect(panel.group.api.location).toEqual({ type: "edge", position: "bottom" });
      expect(button(panel.id).getAttribute("aria-pressed")).toBe("false");
    }

    await update(() => button("output").click());
    expect(api.isEdgeGroupVisible("bottom")).toBe(true);
    expect(button("output").getAttribute("aria-pressed")).toBe("true");
    expect(button("problems").getAttribute("aria-pressed")).toBe("false");
    expect(button("logs").getAttribute("aria-pressed")).toBe("false");

    await update(() => api.removePanel(api.getPanel("editor")!));
    await update(() => resetWorkbenchLayout());
    expect(api.isEdgeGroupVisible("bottom")).toBe(false);
    expect(api.activePanel?.params?.metadata).toEqual({ role: "view", viewId: "project" });
  });

  it("keeps the visible panel selected while editing and collapses or reveals from the status bar", async () => {
    await renderWorkbench();
    await update(() => undefined);
    expect(api.activePanel?.id).toBe("editor");
    expect(button("problems").getAttribute("aria-pressed")).toBe("true");
    expect(button("problems").getAttribute("aria-label")).toBe("panel.problems");
    expect(api.getGroup("bottom")!.model.header.hidden).toBe(true);

    await update(() => button("problems").click());
    expect(api.getEdgeGroup("bottom")!.isCollapsed()).toBe(true);
    expect(api.isEdgeGroupVisible("bottom")).toBe(false);
    expect(button("problems").getAttribute("aria-pressed")).toBe("false");

    await update(() => button("output").click());
    expect(api.getEdgeGroup("bottom")!.isCollapsed()).toBe(false);
    expect(api.isEdgeGroupVisible("bottom")).toBe(true);
    expect(api.getGroup("bottom")!.activePanel?.id).toBe("output");
    expect(button("output").getAttribute("aria-pressed")).toBe("true");
    expect(api.panels).toHaveLength(4);

    await update(() => api.getPanel("editor")!.api.setActive());
    expect(button("output").getAttribute("aria-pressed")).toBe("true");

    await update(() => api.removePanel(api.getPanel("logs")!));
    await update(() => button("logs").click());
    expect(button("logs").getAttribute("aria-pressed")).toBe("true");
    expect(api.panels).toHaveLength(4);
  });

  it("keeps alignment variables local to the status bar and updates changed offsets", async () => {
    await renderWorkbench();
    const workbench = document.querySelector<HTMLElement>("[data-yssbi-workbench]")!;
    const column = workbench.querySelector<HTMLElement>(".dv-shell-middle-column")!;
    const footer = workbench.querySelector("footer")!;
    vi.spyOn(workbench, "getBoundingClientRect").mockReturnValue(new DOMRect(10, 0, 900, 600));
    const bounds = vi.spyOn(column, "getBoundingClientRect");
    bounds.mockReturnValue(new DOMRect(220, 0, 500, 600));

    await update(() => api.getPanel("editor")!.api.setTitle("Resized"));
    expect(footer.style.getPropertyValue("--workbench-center-offset")).toBe("210px");
    expect(footer.style.getPropertyValue("--workbench-center-right-offset")).toBe("190px");
    expect(workbench.style.getPropertyValue("--workbench-center-offset")).toBe("");

    bounds.mockReturnValue(new DOMRect(54, 0, 666, 600));
    await update(() => api.getPanel("editor")!.api.setTitle("Collapsed"));
    expect(footer.style.getPropertyValue("--workbench-center-offset")).toBe("44px");
    expect(footer.style.getPropertyValue("--workbench-center-right-offset")).toBe("190px");
  });

  it("removes a restored collapsed strip and restores native tabs for mixed groups", async () => {
    await renderWorkbench(true);
    await update(() => undefined);
    expect(api.isEdgeGroupVisible("bottom")).toBe(false);
    expect(button("problems").getAttribute("aria-pressed")).toBe("false");

    await update(() => button("output").click());
    const editorGroup = api.getPanel("editor")!.group;
    await update(() => api.getPanel("output")!.api.moveTo({ group: editorGroup }));
    await update(() => button("output").click());
    expect(api.getPanel("output")!.group.id).toBe(editorGroup.id);
    expect(button("output").getAttribute("aria-pressed")).toBe("true");

    await update(() =>
      api
        .getPanel("editor")!
        .api.moveTo({ group: api.groups.find((group) => group.id === "bottom")! }),
    );
    expect(api.getGroup("bottom")!.model.header.hidden).toBe(false);
    expect(api.getGroup("bottom")!.panels.some((panel) => panel.id === "editor")).toBe(true);
  });

  it("opens Details and Assistant from the right corner and follows selection and collapse", async () => {
    await renderWorkbench();
    const corner = document.querySelector('footer [data-workbench-status-panel-tabs="right"]')!;
    expect(
      [...corner.querySelectorAll("button")].map((item) => item.getAttribute("aria-label")),
    ).toEqual(["panel.details", "panel.assistant"]);

    await update(() => button("details").click());
    expect(button("details").getAttribute("aria-pressed")).toBe("true");
    await update(() => button("assistant").click());
    expect(button("assistant").getAttribute("aria-pressed")).toBe("true");
    expect(button("details").getAttribute("aria-pressed")).toBe("false");
    expect(api.panels).toHaveLength(6);

    await update(() => button("assistant").click());
    expect(api.getEdgeGroup("right")!.isCollapsed()).toBe(true);
    expect(button("assistant").getAttribute("aria-pressed")).toBe("false");

    await update(() => button("details").click());
    expect(api.getEdgeGroup("right")!.isCollapsed()).toBe(false);
    expect(button("details").getAttribute("aria-pressed")).toBe("true");
    expect(api.panels).toHaveLength(6);
  });

  it("opens the settings dialog from its status bar icon", async () => {
    await renderWorkbench();
    const settings = document.querySelector<HTMLButtonElement>(
      "footer [data-workbench-status-settings]",
    )!;
    expect(document.querySelector("footer button")).toBe(settings);
    expect(settings.getAttribute("aria-haspopup")).toBe("dialog");
    expect(settings.getAttribute("aria-expanded")).toBe("false");

    await update(() => settings.click());
    expect(workbenchUi.getSnapshot().isSettingsOpen).toBe(true);
    expect(settings.getAttribute("aria-expanded")).toBe("true");
  });
});
