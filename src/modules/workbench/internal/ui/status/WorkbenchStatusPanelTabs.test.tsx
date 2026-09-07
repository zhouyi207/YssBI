// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { DockviewReact, type DockviewApi } from "dockview-react";
import { afterEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import { workbenchDockviewInternal } from "../../dockview/workbenchDockviewInternal";
import { bindWorkbenchStatusBarLayout } from "../../application/workbenchStatusBarLayout";
import type { WorkbenchPanelParams } from "../../dockview/workbenchPanelModel";
import { StatusBar } from "./StatusBar";
import { workbenchUi } from "../../state/ui";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("i18next", () => ({ default: { t: (key: string) => key } }));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function Panel() {
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
                EditorResource: Panel,
                Problems: Panel,
                Output: Panel,
                Logs: Panel,
                Details: Panel,
                Assistant: Panel,
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
          <StatusBar ariaLabel="status" left={[]} right={[]} />
        </TooltipProvider>,
      );
    });
  }

  function button(viewId: string): HTMLButtonElement {
    return document.querySelector(`footer [data-workbench-status-panel="${viewId}"]`)!;
  }

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
