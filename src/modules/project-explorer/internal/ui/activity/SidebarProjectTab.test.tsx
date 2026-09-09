// @vitest-environment happy-dom
import { act } from "react";
import { useActivityPanelExpansion } from "@/features/application/sidebar/useActivityPanelExpansion";
import { createRoot, type Root } from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/app/i18n";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ActivityPanelDocument } from "@/shared/types/domain/activityPanel";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";

import { SidebarProjectTab } from "./SidebarProjectTab";

const browserState = vi.hoisted(() => ({
  current: null as ActivityPanelDocument | null,
}));

vi.mock("@/features/application/sidebar/useActivityPanelDocument", () => ({
  useActivityPanelDocument: () => ({
    document: browserState.current,
    error: null,
    refresh: vi.fn(),
    ...useActivityPanelExpansion("project"),
  }),
}));

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

function renderBrowser() {
  const rows = Object.values(PROJECT_TREE_CATEGORY_IDS).map((id: ProjectTreeCategoryId) =>
    categoryFixture(id, `Projected ${id}`, 0, true),
  );
  rows[3] = {
    ...rows[3],
    tools: [{ id: "importData", label: { key: "contextMenu.sidebar.importData" }, icon: "add" }],
  };
  browserState.current = activityPanelFixture("project", rows);
}

function categoryLabels(host: HTMLElement): string[] {
  return Array.from(host.querySelectorAll("[data-sidebar-tree-category-id]")).map(
    (category) => category.textContent?.trim() ?? "",
  );
}

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("SidebarProjectTab", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    vi.clearAllMocks();
    useSidebarStore.setState({ expandedCategories: {} });
    await i18n.changeLanguage("en-US");
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders Project categories without a search input and preserves category actions", () => {
    renderBrowser();
    act(() =>
      root.render(
        <I18nextProvider i18n={i18n}>
          <TooltipProvider>
            <SidebarProjectTab actions={actions} />
          </TooltipProvider>
        </I18nextProvider>,
      ),
    );

    expect(categoryLabels(host)).toEqual([
      `Projected ${PROJECT_TREE_CATEGORY_IDS.events}`,
      `Projected ${PROJECT_TREE_CATEGORY_IDS.functions}`,
      `Projected ${PROJECT_TREE_CATEGORY_IDS.charts}`,
      `Projected ${PROJECT_TREE_CATEGORY_IDS.data}`,
    ]);
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
  });
});
