// @vitest-environment happy-dom
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
    useSidebarStore.setState({ expandedCategories: {} });
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: "project-1" });
    await i18n.changeLanguage("en-US");
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    useResourceStore.getState().clear();
    useProjectIOStore.setState({ projectInstanceId: null });
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
    expect(host.textContent).toContain("First Event");
    expect(host.querySelector('[role="status"]')).toBeNull();
    act(() =>
      useResourceStore.getState().setSnapshot({
        resources: [{ ...event, loaded: true, exists: false }],
        publicationRevision: 2,
      }),
    );
    expect(host.textContent).not.toContain("First Event");
    expect(host.querySelector('[role="status"]')).toBeNull();
    expect(query).not.toHaveBeenCalled();
  });
});
