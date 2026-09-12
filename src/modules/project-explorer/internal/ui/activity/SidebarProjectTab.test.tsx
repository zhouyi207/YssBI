// @vitest-environment happy-dom
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import {
  startProjectLifecycle,
  captureProjectIdentity,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/app/i18n";
import { TooltipProvider } from "@/components/ui/tooltip";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
import { useProjectIOStore } from "@/features/application/project/projectIOStore";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";
import { useEditorStore } from "@/features/core/editor";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { setInspectionContext } from "@/features/application/editor/rightSidebarActions";
import { workbenchDockviewRead, type WorkbenchEditorPanelInfo } from "@/modules/workbench/public";

import { SidebarProjectTab } from "./SidebarProjectTab";

vi.mock("@dnd-kit/core", () => ({
  useDraggable: () => ({ attributes: {}, listeners: {}, setNodeRef: vi.fn() }),
}));

const actions = {
  onAddEvent: vi.fn(),
  onAddFunction: vi.fn(),
  onAddChart: vi.fn(),
  onImportData: vi.fn(),

  onCategoryContextMenu: vi.fn(),
  onGraphContextMenu: vi.fn(),

  onChartContextMenu: vi.fn(),
  onOpenChart: vi.fn(),
  onDatabaseContextMenu: vi.fn(),
};

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SidebarProjectTab", () => {
  let host: HTMLDivElement;
  let root: Root;
  let activeEditor: WorkbenchEditorPanelInfo | undefined;
  let groupEditors: Map<string, WorkbenchEditorPanelInfo>;
  let dockviewSnapshot: { revision: number; ready: boolean; hydrated: boolean };
  let dockviewListeners: Set<() => void>;

  function publishDockview() {
    dockviewSnapshot = { ...dockviewSnapshot, revision: dockviewSnapshot.revision + 1 };
    for (const listener of dockviewListeners) listener();
  }

  function renderProjectTab() {
    act(() =>
      root.render(
        <I18nextProvider i18n={i18n}>
          <TooltipProvider>
            <SidebarProjectTab actions={actions} />
          </TooltipProvider>
        </I18nextProvider>,
      ),
    );
  }

  function graphRowSelected(name: string): boolean {
    const label = [...host.querySelectorAll("span")].find(
      (element) => element.textContent === name,
    );
    if (!label?.parentElement) throw new Error(`Missing graph row: ${name}`);
    return label.parentElement.classList.contains("bg-[var(--sidebar-item-active)]");
  }

  function installGraphs() {
    const first = buildGraphResourceMeta("event", "events/First.yssbi-event", "First Event");
    const second = buildGraphResourceMeta(
      "function",
      "functions/Second.yssbi-function",
      "Second Function",
    );
    useResourceStore.getState().setResources([first, second]);
    const panel = useSidebarStore.getState().panels.project!;
    useSidebarStore.getState().publishPanels([
      {
        binding: panel.binding,
        snapshot: {
          cursor: "graphs",
          document: activityPanelFixture("project", [
            categoryFixture(PROJECT_TREE_CATEGORY_IDS.events, "Events", 0, true),
            {
              id: first.uri,
              kind: "item",
              depth: 1,
              item: { kind: "graph", path: first.id, name: first.name, graphType: "event" },
            },
            categoryFixture(PROJECT_TREE_CATEGORY_IDS.functions, "Functions", 0, true),
            {
              id: second.uri,
              kind: "item",
              depth: 1,
              item: { kind: "graph", path: second.id, name: second.name, graphType: "function" },
            },
          ]),
        },
      },
    ]);
    activeEditor = {
      panelInstanceId: "first-editor",
      groupId: "first-group",
      component: "EditorResource",
      title: first.name,
      active: true,
      location: { type: "grid" },
      metadata: { role: "editor", resourceRef: first.id, resourceKind: "event" },
    };
    groupEditors.set(activeEditor.groupId, activeEditor);
    useGraphSessionStore.getState().setFocusedSession(activeEditor.groupId, first.id);
    useEditorStore.getState().setDetailFocus({ kind: "event", path: first.id });
    return { first, second };
  }

  beforeEach(async () => {
    vi.clearAllMocks();
    activeEditor = undefined;
    groupEditors = new Map();
    dockviewSnapshot = { revision: 0, ready: true, hydrated: true };
    dockviewListeners = new Set();
    vi.spyOn(workbenchDockviewRead, "getActiveEditorPanel").mockImplementation(() => activeEditor);
    vi.spyOn(workbenchDockviewRead, "getActiveEditorPanelInGroup").mockImplementation((groupId) =>
      groupEditors.get(groupId),
    );
    vi.spyOn(workbenchDockviewRead, "getSnapshot").mockImplementation(() => dockviewSnapshot);
    vi.spyOn(workbenchDockviewRead, "subscribe").mockImplementation((listener) => {
      dockviewListeners.add(listener);
      return () => {
        dockviewListeners.delete(listener);
      };
    });
    useEditorStore.setState({ detailFocus: null });
    useGraphSessionStore.getState().reset();
    startProjectLifecycle("project-1");
    useSidebarStore.setState({ expandedCategories: {}, panels: {} });
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: "project-1" });
    await i18n.changeLanguage("en-US");
    const binding = useSidebarStore
      .getState()
      .bindPanel({ panelId: "project", ...captureProjectIdentity(), locale: "en-US" });
    const rows = Object.values(PROJECT_TREE_CATEGORY_IDS).map((id) =>
      categoryFixture(id, id, 0, true),
    );
    rows[3] = {
      ...rows[3],
      tools: [{ id: "importData", label: { text: "Import Data" }, icon: "add" }],
    };
    useSidebarStore
      .getState()
      .publishPanels([
        { binding, snapshot: { cursor: "c1", document: activityPanelFixture("project", rows) } },
      ]);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    useEditorStore.setState({ detailFocus: null });
    useGraphSessionStore.getState().reset();
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: null });
    clearProjectLifecycle();
    vi.restoreAllMocks();
  });

  it("keeps the current graph highlighted across single, multiple and cleared node selections", () => {
    const { first } = installGraphs();
    renderProjectTab();
    expect(graphRowSelected(first.name)).toBe(true);
    act(() => setInspectionContext(first.id, ["node-1"]));
    expect(useEditorStore.getState().detailFocus).toMatchObject({ kind: "node", id: "node-1" });
    expect(graphRowSelected(first.name)).toBe(true);
    act(() => setInspectionContext(first.id, ["node-1", "node-2"]));
    expect(useEditorStore.getState().detailFocus).toEqual({ kind: "event", path: first.id });
    expect(graphRowSelected(first.name)).toBe(true);
    act(() => setInspectionContext(first.id, ["node-2"]));
    expect(graphRowSelected(first.name)).toBe(true);
    act(() => setInspectionContext(first.id, []));
    expect(graphRowSelected(first.name)).toBe(true);
    act(() => {
      activeEditor = undefined;
      useEditorStore.getState().setDetailFocus({ kind: "nodeDefinition", nodeType: "tests.other" });
      publishDockview();
    });
    expect(graphRowSelected(first.name)).toBe(true);
  });
});
