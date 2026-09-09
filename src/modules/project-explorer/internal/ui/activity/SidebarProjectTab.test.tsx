// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { I18nextProvider } from "react-i18next";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { i18n } from "@/app/i18n";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { ProjectResourceBrowserRow } from "@/features/application/sidebar/projectResourceBrowser";
import type { ProjectTreeCategoryId } from "@/features/core/sidebar/projectTreeState";
import { PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar/projectTreeState";
import type { useProjectResourceBrowser } from "@/features/application/sidebar/useProjectResourceBrowser";
import { SidebarProjectTab } from "./SidebarProjectTab";

const browserState = vi.hoisted(() => ({
  current: null as ReturnType<typeof useProjectResourceBrowser> | null,
}));

vi.mock("@/features/application/sidebar/useProjectResourceBrowser", () => ({
  useProjectResourceBrowser: () => browserState.current,
}));

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number; getItemKey: (index: number) => string | number }) => ({
    getTotalSize: () => options.count * 32,
    getVirtualItems: () =>
      Array.from({ length: options.count }, (_, index) => ({
        index,
        key: options.getItemKey(index),
        start: index * 32,
        size: 32,
      })),
    measureElement: vi.fn(),
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

function categoryRow(categoryId: ProjectTreeCategoryId): ProjectResourceBrowserRow {
  const level = 0;
  return {
    kind: "category",
    rowKey: `category:${categoryId}`,
    categoryId,
    level,
    label: `Projected ${categoryId}`,
    expanded: true,
  };
}

function renderBrowser() {
  const categoryIds = [
    PROJECT_TREE_CATEGORY_IDS.events,
    PROJECT_TREE_CATEGORY_IDS.functions,
    PROJECT_TREE_CATEGORY_IDS.charts,
    PROJECT_TREE_CATEGORY_IDS.data,
  ];
  const rows = categoryIds.map(categoryRow);

  browserState.current = {
    rows,
    activeGraph: null,
    setCategoryExpanded: vi.fn(),
  } as ReturnType<typeof useProjectResourceBrowser>;
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
    expect(browserState.current!.setCategoryExpanded).toHaveBeenCalledWith(
      PROJECT_TREE_CATEGORY_IDS.data,
      false,
    );
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
