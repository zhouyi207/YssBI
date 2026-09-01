// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { DockviewReact, type DockviewApi, type DockviewGroupPanel } from "dockview-react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import type { WorkbenchPanelParams } from "@/features/core/dockview";

const WORKBENCH_DOCKVIEW_CSS = readFileSync(
  join(process.cwd(), "src/app/workbench-dockview.css"),
  "utf8",
);

const mocks = vi.hoisted(() => ({
  dirty: false,
  groupPanels: [] as Array<{ panelInstanceId: string }>,
  requestCloseWorkbenchPanel: vi.fn(() => Promise.resolve(false)),
  requestCloseWorkbenchPanels: vi.fn(() => Promise.resolve(false)),
  requestCloseEditorPanel: vi.fn(() => Promise.resolve(false)),
  listGroupPanels: vi.fn(),
  buildEditorPanelTabMenu: vi.fn(() => [
    {
      items: [{ id: "document-action", label: "document-action" }],
    },
  ]),
}));

vi.mock("@/features/application/editor/workbenchPanelClose", () => ({
  requestCloseWorkbenchPanel: mocks.requestCloseWorkbenchPanel,
  requestCloseWorkbenchPanels: mocks.requestCloseWorkbenchPanels,
}));

vi.mock("@/features/application/editor/editorPanelCloseCommands", () => ({
  requestCloseEditorPanel: mocks.requestCloseEditorPanel,
}));

vi.mock("@/features/application/editor/editorPanelTabMenu", () => ({
  buildEditorPanelTabMenu: mocks.buildEditorPanelTabMenu,
}));

vi.mock("@/features/core/dockview", () => ({
  isWorkbenchActivityViewId: (viewId: string) =>
    ["project", "nodes", "data", "commands"].includes(viewId),
  isWorkbenchPersistentViewMetadata: (metadata: { role: string; viewId?: string }) =>
    metadata.role === "view" && metadata.viewId === "details",
  workbenchDockviewRead: {
    listGroupPanels: mocks.listGroupPanels,
  },
}));

vi.mock("@/features/core/resource", () => ({
  resourceKey: ({ id, kind }: { id: string; kind: string }) => `${kind}:${id}`,
}));

vi.mock("@/features/core/resource/read", () => ({
  useResourceRead: (
    selector: (state: { documents: Record<string, { dirty: boolean }> }) => unknown,
  ) =>
    selector({
      documents: {
        "event:events/Main.yssbi-event": { dirty: mocks.dirty },
      },
    }),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { RootPanelTabRenderer } from "./RootPanelTabRenderer";

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
      pinned: true,
    },
  };
}

function viewParams(): WorkbenchPanelParams {
  return { metadata: { role: "view", viewId: "logs" } };
}

function detailsParams(): WorkbenchPanelParams {
  return { metadata: { role: "view", viewId: "details" } };
}

const fixedViewCases = [
  { viewId: "project", component: "Project", titleKey: "activityBar.project" },
  { viewId: "nodes", component: "Nodes", titleKey: "activityBar.nodes" },
  { viewId: "data", component: "Data", titleKey: "activityBar.data" },
  { viewId: "commands", component: "Commands", titleKey: "activityBar.commands" },
  { viewId: "details", component: "Details", titleKey: "panel.details" },
  { viewId: "inspect", component: "Inspect", titleKey: "panel.inspect" },
  { viewId: "logs", component: "Logs", titleKey: "panel.logs" },
  { viewId: "output", component: "Output", titleKey: "panel.output" },
  { viewId: "diagnostics", component: "Diagnostics", titleKey: "panel.diagnostics" },
] as const;

function resultParams(): WorkbenchPanelParams {
  return {
    metadata: {
      role: "result",
      resultKey: "output:main",
      resultId: "result-1",
      title: "Distribution",
      presentation: { kind: "inspector" },
      source: null,
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
    mocks.groupPanels = [];
    mocks.listGroupPanels.mockImplementation(() => mocks.groupPanels);
    api = null;
    host = document.createElement("div");
    host.dataset.yssbiRootDockview = "";
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
              Data: TestPanel,
              Commands: TestPanel,
              Details: TestPanel,
              Inspect: TestPanel,
              Logs: TestPanel,
              Output: TestPanel,
              Diagnostics: TestPanel,
              Result: TestPanel,
            }}
            defaultTabComponent={RootPanelTabRenderer}
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

  it("localizes every fixed workbench view tab", () => {
    renderDockview((readyApi) => {
      fixedViewCases.forEach(({ viewId, component }) =>
        readyApi.addPanel<WorkbenchPanelParams>({
          id: `${viewId}-a`,
          component,
          title: `fallback-${viewId}`,
          params: { metadata: { role: "view", viewId } },
        }),
      );
    });

    fixedViewCases.forEach(({ viewId, titleKey }) => {
      const content = host.querySelector<HTMLElement>(`[data-panel-instance-id="${viewId}-a"]`);
      const title = ["project", "nodes", "data", "commands"].includes(viewId)
        ? content?.textContent
        : content?.querySelector("[data-workbench-tab-title]")?.textContent;
      expect(title).toContain(titleKey);
    });
  });

  it("shows canonical editor chrome and routes close and middle-click without native removal", () => {
    mocks.dirty = true;
    renderDockview((readyApi) => {
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "editor-a",
        component: "EditorResource",
        title: "Main",
        params: editorParams(),
      });
    });

    const content = host.querySelector<HTMLElement>('[data-panel-instance-id="editor-a"]')!;
    expect(content.querySelector("[data-workbench-tab-title]")?.textContent).toBe("Main");
    expect(content.querySelector('[data-workbench-tab-icon="event"]')).not.toBeNull();
    expect(content.querySelector("[data-workbench-tab-dirty]")).not.toBeNull();

    const closeButton = content.querySelector<HTMLButtonElement>("[data-workbench-tab-close]")!;
    const closeEvent = new MouseEvent("click", {
      button: 0,
      bubbles: true,
      cancelable: true,
    });
    act(() => closeButton.dispatchEvent(closeEvent));

    expect(closeEvent.defaultPrevented).toBe(true);
    expect(mocks.requestCloseEditorPanel).toHaveBeenNthCalledWith(1, "editor-a");
    expect(api?.getPanel("editor-a")).toBeDefined();

    const tab = tabHeaderHost("editor-a");
    const pointerDown = new MouseEvent("pointerdown", {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    const pointerUp = new MouseEvent("pointerup", {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    act(() => {
      tab.dispatchEvent(pointerDown);
      tab.dispatchEvent(pointerUp);
    });

    expect(pointerDown.defaultPrevented).toBe(true);
    expect(pointerUp.defaultPrevented).toBe(true);
    expect(mocks.requestCloseEditorPanel).toHaveBeenNthCalledWith(2, "editor-a");
    expect(api?.getPanel("editor-a")).toBeDefined();
  });

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

  it("keeps the permanent Details tab open without a close affordance", () => {
    renderDockview((readyApi) => {
      readyApi.addPanel<WorkbenchPanelParams>({
        id: "details-a",
        component: "Details",
        title: "Details",
        params: detailsParams(),
      });
    });

    const content = host.querySelector<HTMLElement>('[data-panel-instance-id="details-a"]')!;
    expect(content.querySelector("[data-workbench-tab-title]")?.textContent).toBe("panel.details");
    expect(content.querySelector('[data-workbench-tab-icon="details"]')).not.toBeNull();
    expect(content.querySelector("[data-workbench-tab-close]")).toBeNull();

    const tab = tabShell("details-a");
    const pointerDown = new MouseEvent("pointerdown", {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    const pointerUp = new MouseEvent("pointerup", {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    act(() => {
      tab.dispatchEvent(pointerDown);
      tab.dispatchEvent(pointerUp);
    });

    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
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
        id: "diagnostics-a",
        component: "Logs",
        title: "Diagnostics",
        params: { metadata: { role: "view", viewId: "diagnostics" } },
        inactive: true,
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
    });

    const bottomGroup = api?.getEdgeGroup("bottom");
    if (!bottomGroup) throw new Error("Missing bottom edge group");

    act(() => bottomGroup.collapse());

    expect(host.querySelectorAll('[data-workbench-tab-edge-collapsed="true"]')).toHaveLength(2);

    act(() => tabShell("diagnostics-a").click());

    expect(bottomGroup.isCollapsed()).toBe(false);
    expect(api?.activePanel?.id).toBe("diagnostics-a");
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
        id: "diagnostics-a",
        component: "Logs",
        title: "Diagnostics",
        params: { metadata: { role: "view", viewId: "diagnostics" } },
        inactive: true,
        position: { referenceGroup: bottomGroup, direction: "within" },
      });
    });

    const outputTab = tabShell("output-a");
    const diagnosticsTab = tabShell("diagnostics-a");

    expect(getComputedStyle(outputTab).margin).toBe(getComputedStyle(diagnosticsTab).margin);

    act(() => diagnosticsTab.click());

    expect(getComputedStyle(outputTab).margin).toBe(getComputedStyle(diagnosticsTab).margin);
  });

  it("collapses the left Activity edge when its active tab is clicked again", () => {
    renderDockview((readyApi) => {
      readyApi.addEdgeGroup("left", {
        id: "activity-edge",
        initialSize: 240,
      });
      const activityGroup = readyApi.groups.find((group) => group.id === "activity-edge");
      if (!activityGroup) throw new Error("Missing activity edge group");

      readyApi.addPanel<WorkbenchPanelParams>({
        id: "project-a",
        component: "Details",
        title: "Project",
        params: { metadata: { role: "view", viewId: "project" } },
        position: { referenceGroup: activityGroup, direction: "within" },
      });
    });

    const leftGroup = api?.getEdgeGroup("left");
    if (!leftGroup) throw new Error("Missing left edge group");
    const activityTab = host.querySelector<HTMLElement>('[data-panel-instance-id="project-a"]');
    if (!activityTab) throw new Error("Missing project activity tab");

    expect(leftGroup.isCollapsed()).toBe(false);

    act(() => activityTab.click());

    expect(leftGroup.isCollapsed()).toBe(true);
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
    mocks.groupPanels = [{ panelInstanceId: "logs-a" }, { panelInstanceId: "result-a" }];

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

    expect(mocks.listGroupPanels).toHaveBeenCalledOnce();
    expect(mocks.listGroupPanels).toHaveBeenCalledWith(result?.group.id);
    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledOnce();
    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledWith(["logs-a", "result-a"]);
    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
  });
});
