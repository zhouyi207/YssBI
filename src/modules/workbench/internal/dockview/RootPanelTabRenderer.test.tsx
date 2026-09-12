// @vitest-environment happy-dom
import { resultReferenceFixture, resultLeaseIdFixture } from "@/tests/helpers/resultFixture";

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DockviewReact, type DockviewApi, type DockviewGroupPanel } from "dockview-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelParams } from "./index";

const WORKBENCH_DOCKVIEW_CSS = readFileSync(
  join(process.cwd(), "src/app/workbench-dockview.css"),
  "utf8",
);

const mocks = vi.hoisted(() => ({
  dirty: false,
  requestCloseWorkbenchPanel: vi.fn(() => Promise.resolve(false)),
  requestCloseWorkbenchGroup: vi.fn(() => Promise.resolve(false)),
  requestCloseEditorPanel: vi.fn(() => Promise.resolve(false)),
  buildEditorPanelTabMenu: vi.fn(() => [
    {
      items: [{ id: "document-action", label: "document-action" }],
    },
  ]),
}));

vi.mock("@/features/application/editor/workbenchPanelClose", () => ({
  requestCloseWorkbenchPanel: mocks.requestCloseWorkbenchPanel,
  requestCloseWorkbenchGroup: mocks.requestCloseWorkbenchGroup,
}));

vi.mock("@/features/application/editor/editorPanelCloseCommands", () => ({
  requestCloseEditorPanel: mocks.requestCloseEditorPanel,
}));

vi.mock("@/features/application/editor/editorPanelTabMenu", () => ({
  buildEditorPanelTabMenu: mocks.buildEditorPanelTabMenu,
}));

vi.mock("./index", () => ({
  isWorkbenchActivityViewId: (viewId: string) =>
    ["project", "nodes", "commands", "plugins"].includes(viewId),
  isWorkbenchPersistentViewMetadata: (metadata: { role: string; viewId?: string }) =>
    metadata.role === "view" && metadata.viewId === "details",
}));

vi.mock("@/features/application/editor/useEditorPanelDirty", () => ({
  useEditorPanelDirty: () => mocks.dirty,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { rootPanelTabRenderer } from "@/app/windows/workbench/rootPanelTabRenderer";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function TestPanel() {
  return null;
}

function editorParams(): WorkbenchPanelParams {
  return {
    metadata: {
      role: "editor",
      resourceRef: "events/Main.yssbi-event",
      resourceKind: "event",
    },
  };
}

function viewParams(): WorkbenchPanelParams {
  return { metadata: { role: "view", viewId: "logs" } };
}

function resultParams(): WorkbenchPanelParams {
  return {
    metadata: {
      role: "result",
      leaseId: resultLeaseIdFixture(1),
      reference: resultReferenceFixture("1"),
      title: "Distribution",
      presentation: { kind: "inspector" },
    },
  };
}

describe("RootPanelTabRenderer", () => {
  let host: HTMLDivElement;
  let root: Root;
  let api: DockviewApi | null;
  let workbenchStyle: HTMLStyleElement;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.dirty = false;
    api = null;
    host = document.createElement("div");
    host.dataset.yssbiRootDockview = "";
    host.style.setProperty("--accent-color", "rgb(49, 94, 222)");
    document.body.appendChild(host);
    workbenchStyle = document.createElement("style");
    workbenchStyle.textContent = WORKBENCH_DOCKVIEW_CSS;
    document.head.appendChild(workbenchStyle);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    workbenchStyle.remove();
    document.body.replaceChildren();
  });

  function renderDockview(initialize: (readyApi: DockviewApi) => void): void {
    act(() =>
      root.render(
        <div style={{ width: 640, height: 480 }}>
          <DockviewReact
            components={{
              EditorResource: TestPanel,
              Project: TestPanel,
              Nodes: TestPanel,
              Commands: TestPanel,
              Plugins: TestPanel,
              Details: TestPanel,
              Inspect: TestPanel,
              Logs: TestPanel,
              Output: TestPanel,
              Problems: TestPanel,
              Result: TestPanel,
            }}
            defaultTabComponent={rootPanelTabRenderer}
            onReady={({ api: readyApi }) => {
              api = readyApi;
              initialize(readyApi);
            }}
          />
        </div>,
      ),
    );
  }

  function tabShell(panelInstanceId: string): HTMLElement {
    const content = host.querySelector<HTMLElement>(
      `[data-panel-instance-id="${panelInstanceId}"]`,
    );
    const tab = content?.closest<HTMLElement>(".dv-tab");
    if (!tab) throw new Error(`Missing tab ${panelInstanceId}`);
    return tab;
  }

  function tabHeaderHost(panelInstanceId: string): HTMLElement {
    const content = host.querySelector<HTMLElement>(
      `[data-panel-instance-id="${panelInstanceId}"]`,
    );
    const headerHost = content?.parentElement;
    if (!headerHost) throw new Error(`Missing tab header host ${panelInstanceId}`);
    return headerHost;
  }

  it("keeps the existing editor document context menu", () => {
    renderDockview((readyApi) => {
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-a",
        component: "EditorResource",
        title: "Main",
        params: editorParams(),
      });
    });

    const panel = api?.getPanel("editor-a");
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 21,
      clientY: 34,
    });
    act(() => tabHeaderHost("editor-a").dispatchEvent(event));

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.buildEditorPanelTabMenu).toHaveBeenCalledWith(
      {
        panelInstanceId: "editor-a",
        groupId: panel?.group.id,
      },
      expect.any(Function),
    );
    expect(document.querySelector('[role="menu"]')?.textContent).toContain("document-action");
  });

  it("keeps the editor document context menu after splitting the tab into a new group", () => {
    let sourceGroupId: string | undefined;

    renderDockview((readyApi) => {
      const movedPanel = readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-a",
        component: "EditorResource",
        title: "Main",
        params: editorParams(),
      });
      sourceGroupId = movedPanel.group.id;
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-b",
        component: "EditorResource",
        title: "Secondary",
        params: editorParams(),
        position: { referenceGroup: movedPanel.group, direction: "within" },
      });
    });

    const movedPanel = api?.getPanel("editor-a");
    if (!movedPanel || !sourceGroupId) throw new Error("Missing moved panel");

    act(() => {
      movedPanel.api.moveTo({
        group: movedPanel.group,
        position: "right",
      });
    });

    const splitPanel = api?.getPanel("editor-a");
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 21,
      clientY: 34,
    });
    act(() => tabHeaderHost("editor-a").dispatchEvent(event));

    expect(splitPanel?.group.id).not.toBe(sourceGroupId);
    expect(event.defaultPrevented).toBe(true);
    expect(mocks.buildEditorPanelTabMenu).toHaveBeenCalledWith(
      {
        panelInstanceId: "editor-a",
        groupId: splitPanel?.group.id,
      },
      expect.any(Function),
    );
    expect(document.querySelector('[role="menu"]')?.textContent).toContain("document-action");
  });

  it("treats collapsed edge tabs as unselected and expands the selected tab", () => {
    renderDockview((readyApi) => {
      readyApi.addEdgeGroup("bottom", {
        id: "bottom-edge",
        initialSize: 180,
      });
      const bottomGroup = readyApi.groups.find((group) => group.id === "bottom-edge");
      if (!bottomGroup) throw new Error("Missing bottom edge group");

      readyApi.addPanel<WorkbenchPanelParams>({
        id: "output-a",
        component: "Logs",
        title: "Output",
        params: { metadata: { role: "view", viewId: "output" } },
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "problems-a",
        component: "Logs",
        title: "Problems",
        params: { metadata: { role: "view", viewId: "problems" } },
        inactive: true,
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
    });

    const bottomGroup = api?.getEdgeGroup("bottom");
    if (!bottomGroup) throw new Error("Missing bottom edge group");

    act(() => bottomGroup.collapse());

    expect(host.querySelectorAll('[data-workbench-tab-edge-collapsed="true"]')).toHaveLength(2);

    act(() => tabShell("problems-a").click());

    expect(bottomGroup.isCollapsed()).toBe(false);
    expect(api?.activePanel?.id).toBe("problems-a");
    expect(host.querySelectorAll('[data-workbench-tab-edge-collapsed="true"]')).toHaveLength(0);
  });

  it("follows edge collapse state after a panel moves out of an edge group", () => {
    let centralGroup: DockviewGroupPanel | undefined;

    renderDockview((readyApi) => {
      const centralPanel = readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-b",
        component: "EditorResource",
        title: "Secondary",
        params: editorParams(),
      });
      centralGroup = centralPanel.group;
      readyApi.addEdgeGroup("bottom", {
        id: "bottom-edge",
        initialSize: 180,
      });
      const bottomGroup = readyApi.groups.find((group) => group.id === "bottom-edge");
      if (!bottomGroup) throw new Error("Missing bottom edge group");
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-a",
        component: "EditorResource",
        title: "Main",
        params: editorParams(),
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
    });

    const panel = api?.getPanel("editor-a");
    const bottomGroup = api?.getEdgeGroup("bottom");
    if (!panel || !bottomGroup || !centralGroup) throw new Error("Missing panel groups");

    act(() => bottomGroup.collapse());

    expect(host.querySelector('[data-workbench-tab-edge-collapsed="true"]')).not.toBeNull();

    act(() => panel.api.moveTo({ group: centralGroup }));

    expect(host.querySelector('[data-workbench-tab-edge-collapsed="true"]')).toBeNull();
  });

  it("keeps bottom edge tab geometry stable when activation changes", () => {
    renderDockview((readyApi) => {
      readyApi.addEdgeGroup("bottom", {
        id: "bottom-edge",
        initialSize: 180,
      });
      const bottomGroup = readyApi.groups.find((group) => group.id === "bottom-edge");
      if (!bottomGroup) throw new Error("Missing bottom edge group");

      readyApi.addPanel<WorkbenchPanelParams>({
        id: "output-a",
        component: "Logs",
        title: "Output",
        params: { metadata: { role: "view", viewId: "output" } },
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "problems-a",
        component: "Logs",
        title: "Problems",
        params: { metadata: { role: "view", viewId: "problems" } },
        inactive: true,
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
    });

    const outputTab = tabShell("output-a");
    const problemsTab = tabShell("problems-a");

    expect(getComputedStyle(outputTab).margin).toBe(getComputedStyle(problemsTab).margin);

    act(() => problemsTab.click());

    expect(getComputedStyle(outputTab).margin).toBe(getComputedStyle(problemsTab).margin);
  });

  it("pins native Plugins with shared Activity styling and collapse, expand, and switch behavior", () => {
    renderDockview((readyApi) => {
      readyApi.addEdgeGroup("left", {
        id: "activity-edge",
        initialSize: 240,
      });
      const activityGroup = readyApi.groups.find((group) => group.id === "activity-edge");
      if (!activityGroup) throw new Error("Missing activity edge group");

      readyApi.addPanel<WorkbenchPanelParams>({
        id: "project-a",
        component: "Project",
        title: "Project",
        params: { metadata: { role: "view", viewId: "project" } },
        position: { referenceGroup: activityGroup, direction: "within" },
      });
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "plugins-a",
        component: "Plugins",
        title: "Plugins",
        params: { metadata: { role: "view", viewId: "plugins" } },
        position: { referenceGroup: activityGroup, direction: "within" },
        inactive: true,
      });
    });

    const leftGroup = api?.getEdgeGroup("left");
    if (!leftGroup) throw new Error("Missing left edge group");
    const project = host.querySelector<HTMLElement>('[data-panel-instance-id="project-a"]')!;
    const plugins = host.querySelector<HTMLElement>('[data-panel-instance-id="plugins-a"]')!;
    const projectTab = tabShell("project-a");
    const pluginsTab = tabShell("plugins-a");
    const iconStyle = (tab: HTMLElement) =>
      getComputedStyle(tab.querySelector<HTMLElement>("[data-workbench-activity-icon]")!);
    const selectedColor = iconStyle(project).color;
    const selectedBackground = iconStyle(project).backgroundColor;

    expect(getComputedStyle(pluginsTab).display).not.toBe("none");
    expect(getComputedStyle(pluginsTab).order).toBe("1");
    expect(getComputedStyle(pluginsTab).marginTop).toBe("auto");
    expect(getComputedStyle(pluginsTab).width).toBe(getComputedStyle(projectTab).width);
    expect(getComputedStyle(pluginsTab).height).toBe(getComputedStyle(projectTab).height);

    act(() => plugins.click());
    expect(leftGroup.isCollapsed()).toBe(false);
    expect(api?.activePanel?.id).toBe("plugins-a");
    expect(iconStyle(plugins).color).toBe(selectedColor);
    expect(iconStyle(plugins).backgroundColor).toBe(selectedBackground);

    act(() => plugins.click());
    expect(leftGroup.isCollapsed()).toBe(true);
    expect(plugins.dataset.workbenchTabEdgeCollapsed).toBe("true");

    act(() => plugins.click());
    expect(leftGroup.isCollapsed()).toBe(false);
    expect(plugins.dataset.workbenchTabEdgeCollapsed).toBeUndefined();

    act(() => project.click());
    expect(leftGroup.isCollapsed()).toBe(false);
    expect(api?.activePanel?.id).toBe("project-a");
    act(() => project.click());
    expect(leftGroup.isCollapsed()).toBe(true);
    act(() => plugins.click());
    expect(leftGroup.isCollapsed()).toBe(false);
    expect(api?.activePanel?.id).toBe("plugins-a");
  });

  it("closes one physical mixed group through one batch request", () => {
    renderDockview((readyApi) => {
      const logs = readyApi.addPanel<WorkbenchPanelParams>({
        id: "logs-a",
        component: "Logs",
        title: "Logs",
        params: viewParams(),
      });
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "result-a",
        component: "Result",
        title: "Distribution",
        params: resultParams(),
        position: { referencePanel: logs.id, direction: "within" },
      });
    });
    const result = api?.getPanel("result-a");
    const event = new MouseEvent("contextmenu", {
      bubbles: true,
      cancelable: true,
      clientX: 55,
      clientY: 89,
    });
    act(() => tabHeaderHost("result-a").dispatchEvent(event));

    const closeGroupItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (item) => item.textContent?.includes("tabBar.closeGroup"),
    );
    expect(closeGroupItem).toBeDefined();
    act(() => closeGroupItem?.click());

    expect(mocks.requestCloseWorkbenchGroup).toHaveBeenCalledOnce();
    expect(mocks.requestCloseWorkbenchGroup).toHaveBeenCalledWith(result?.group.id);
    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
  });
});
