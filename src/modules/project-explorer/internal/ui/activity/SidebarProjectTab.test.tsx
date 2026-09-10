// @vitest-environment happy-dom
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import { parseActivityPanelUpdate } from "@/shared/types/dto/activityPanel";
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
import * as activityService from "@/services/workbench/activityPanelService";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";

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

function categoryIds(host: HTMLElement): Array<string | null> {
  return Array.from(host.querySelectorAll("[data-sidebar-tree-category-id]")).map((category) =>
    category.getAttribute("data-sidebar-tree-category-id"),
  );
}

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SidebarProjectTab", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    vi.clearAllMocks();
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
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: null });
    clearProjectLifecycle();
    vi.restoreAllMocks();
  });

  it("renders Project categories without a search input and preserves category actions", () => {
    const query = vi.spyOn(activityService, "getActivityPanelDocument");
    act(() =>
      root.render(
        <I18nextProvider i18n={i18n}>
          <TooltipProvider>
            <SidebarProjectTab actions={actions} />
          </TooltipProvider>
        </I18nextProvider>,
      ),
    );

    expect(categoryIds(host)).toEqual(Object.values(PROJECT_TREE_CATEGORY_IDS));
    expect(host.querySelector("input")).toBeNull();
    expect(host.querySelector("[data-sidebar-tree-search]")).toBeNull();

    const data = host.querySelector<HTMLButtonElement>(
      `[data-sidebar-tree-category-id="${PROJECT_TREE_CATEGORY_IDS.data}"]`,
    )!;
    act(() => data.click());
    expect(
      useSidebarStore.getState().expandedCategories.project?.[PROJECT_TREE_CATEGORY_IDS.data],
    ).toBe(false);
    act(() => data.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true })));
    expect(actions.onCategoryContextMenu).toHaveBeenCalledWith(
      expect.anything(),
      PROJECT_TREE_CATEGORY_IDS.data,
    );
    const importButton = host.querySelector<HTMLButtonElement>('[aria-label="Import Data"]')!;
    act(() => importButton.click());
    expect(actions.onImportData).toHaveBeenCalledOnce();

    const event = buildGraphResourceMeta("event", "events/First.yssbi-event", "First Event");
    act(() =>
      useResourceStore.getState().setSnapshot({ resources: [event], publicationRevision: 1 }),
    );
    expect(host.textContent).not.toContain("First Event");
    const panel = useSidebarStore.getState().panels.project!;
    const inserted = parseActivityPanelUpdate(
      {
        kind: "patch",
        baseCursor: "c1",
        cursor: "c2",
        patch: {},
        operations: [
          {
            op: "insert",
            afterId: "project.events",
            row: {
              id: event.uri,
              kind: "item",
              depth: 1,
              item: { kind: "graph", path: event.id, name: "First Event", graphType: "event" },
            },
          },
        ],
      },
      panel.snapshot,
    )!;
    act(() =>
      useSidebarStore.getState().publishPanels([{ binding: panel.binding, snapshot: inserted }]),
    );
    expect(host.textContent).toContain("First Event");
    expect(host.querySelector('[role="status"]')).toBeNull();
    act(() =>
      useResourceStore.getState().setSnapshot({
        resources: [{ ...event, loaded: true, exists: false }],
        publicationRevision: 2,
      }),
    );
    expect(host.textContent).toContain("First Event");
    const removed = parseActivityPanelUpdate(
      {
        kind: "patch",
        baseCursor: "c2",
        cursor: "c3",
        patch: {},
        operations: [{ op: "remove", id: event.uri }],
      },
      inserted,
    )!;
    act(() =>
      useSidebarStore.getState().publishPanels([{ binding: panel.binding, snapshot: removed }]),
    );
    expect(host.textContent).not.toContain("First Event");
    expect(host.querySelector('[role="status"]')).toBeNull();
    expect(query).not.toHaveBeenCalled();
  });
});
