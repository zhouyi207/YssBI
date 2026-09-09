// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LocalizedNodeCatalogState } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import * as projectHydration from "@/features/application/project/projectHydration";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { SidebarDataRow } from "@/modules/project-explorer/internal/ui/activity/SidebarDataRow";
import { SidebarProjectTab } from "@/modules/project-explorer/internal/ui/activity/SidebarProjectTab";
import { useSidebarStore, PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  catalogState: null as LocalizedNodeCatalogState | null,
  draggableInputs: [] as Array<{ data: unknown; disabled?: boolean }>,
  dragPointerDown: vi.fn(),
  revealDetails: vi.fn(),
}));

vi.mock("@dnd-kit/core", () => ({
  useDraggable: (input: { data: unknown; disabled?: boolean }) => {
    mocks.draggableInputs.push(input);
    return {
      attributes: {},
      listeners: { onPointerDown: mocks.dragPointerDown },
      setNodeRef: vi.fn(),
    };
  },
}));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number; getItemKey: (index: number) => string | number }) => ({
    getTotalSize: () => options.count * 28,
    getVirtualItems: () =>
      Array.from({ length: options.count }, (_, index) => ({
        index,
        key: options.getItemKey(index),
        start: index * 28,
        size: 28,
      })),
    measureElement: vi.fn(),
  }),
}));
vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => mocks.catalogState,
}));
vi.mock("@/features/application/editor/rightSidebarActions", () => ({
  revealDetails: mocks.revealDetails,
}));
vi.mock("@/features/application/window", () => ({ openDatabaseEditorWindow: vi.fn() }));
vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@/components/ui/tooltip", () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
  TooltipContent: ({ children }: { children: React.ReactNode }) => children,
}));

const databasePath = "databases/sales / . # 数据";

const databaseSource: NodeCreationDescriptor = {
  kind: "resourceBound",
  nodeTypeId: "yssbi.dataframe.source.get",
  resourcePath: databasePath,
  resourceRevision: 4,
  createArgs: { kind: "database" },
};

function item(title: string, descriptor: NodeCreationDescriptor) {
  return {
    nodeTypeId: descriptor.nodeTypeId,
    title,
    documentation: null,
    categoryId: "resources",
    iconId: "resource",
    styleId: "default",
    aliases: [],
    technicalTerms: [],
    backendSearchText: [title],
    resourceNames: [title],
    ports: [],
    parameters: [],
    resourcePath: descriptor.kind === "resourceBound" ? descriptor.resourcePath : undefined,
    resourceRevision: descriptor.kind === "resourceBound" ? descriptor.resourceRevision : undefined,
    creation: descriptor,
  };
}

function catalogState(
  status: LocalizedNodeCatalogState["status"] = "ready",
  items = [item("Sales", databaseSource)],
): LocalizedNodeCatalogState {
  return {
    status,
    error: status === "error" ? { code: "catalog_response_stale", incidentId: null } : null,
    catalog: {
      projectInstanceId: "project-1",
      registryFingerprint: "registry-1",
      resourcePublicationRevision: 8,
      locale: "en-US",
      categories: [],
      items,
    },
    searchIndex: null,
    refresh: vi.fn(),
  };
}

describe("resource sidebar rows", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.draggableInputs.length = 0;
    mocks.catalogState = catalogState();
    mocks.revealDetails.mockResolvedValue(undefined);
    useDatabaseStore.getState().clear();
    vi.spyOn(projectHydration, "refreshProjectResourceIndex").mockResolvedValue(true);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderDatabase(resourcePath: string | null = databasePath, data: unknown = {}) {
    act(() =>
      root.render(
        <SidebarDataRow
          id="database-id"
          resourcePath={resourcePath ?? undefined}
          name="Sales"
          data={data}
          onContextMenu={vi.fn()}
        />,
      ),
    );
  }

  it("shows live data in the Project tree with category expansion and no search input", () => {
    useSidebarStore.getState().setProjectTreeCategoryExpanded(PROJECT_TREE_CATEGORY_IDS.data, true);
    useDatabaseStore.getState().addDatabase("database-id", {
      id: "database-id",
      name: "Sales",
      resourcePath: databasePath,
    });
    act(() =>
      root.render(
        <SidebarProjectTab
          actions={{
            onAddEvent: vi.fn(),
            onAddFunction: vi.fn(),
            onAddChart: vi.fn(),
            onImportData: vi.fn(),
            onCategoryContextMenu: vi.fn(),
            onGraphContextMenu: vi.fn(),
            onChartContextMenu: vi.fn(),
            onOpenChart: vi.fn(),
            onDatabaseContextMenu: vi.fn(),
          }}
        />,
      ),
    );

    expect(host.textContent).toContain("Sales");
    expect(host.querySelector("input")).toBeNull();
    const dataCategory = () =>
      host.querySelector<HTMLButtonElement>('[data-sidebar-tree-category-id="project.data"]')!;
    act(() => dataCategory().click());
    expect(host.textContent).not.toContain("Sales");

    act(() => dataCategory().click());
    expect(host.textContent).toContain("Sales");
    expect(dataCategory().disabled).toBe(false);
    expect(host.querySelectorAll("[data-sidebar-tree-category-id]")).toHaveLength(4);
    expect(
      useSidebarStore.getState().projectTreeExpandedCategories[PROJECT_TREE_CATEGORY_IDS.data],
    ).toBe(true);

    act(() => useDatabaseStore.getState().updateDatabase("database-id", { name: "Sales updated" }));
    expect(host.textContent).toContain("Sales updated");
  });

  it("forwards pointer down to the dnd-kit drag listener", () => {
    renderDatabase();
    const row = host.firstElementChild;

    act(() => row?.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));

    expect(mocks.dragPointerDown).toHaveBeenCalledOnce();
  });

  it("uses the exact current database source descriptor", () => {
    renderDatabase();

    const input = mocks.draggableInputs[mocks.draggableInputs.length - 1];
    expect(input).toMatchObject({
      disabled: false,
      data: { type: "node-template", template: { title: "Sales", descriptor: databaseSource } },
    });
    const dragData = input?.data as { template?: { descriptor?: unknown } };
    expect(dragData.template?.descriptor).toBe(databaseSource);
  });

  it("explicitly reveals Details for database row clicks", async () => {
    renderDatabase();
    await act(async () => {
      host.firstElementChild?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.revealDetails.mock.calls.map(([focus]) => focus)).toEqual([
      { kind: "data", id: "database-id" },
    ]);
  });

  it("shows the localized load failure tooltip from machine state", () => {
    renderDatabase(databasePath, { loadFailed: true });

    expect(host.textContent).toContain("sidebar.dataLoadFailed");
  });

  it("does not treat a legacy raw load error as failure state", () => {
    renderDatabase(databasePath, { loadError: "sensitive backend failure" });

    expect(host.textContent).not.toContain("sidebar.dataLoadFailed");
    expect(host.textContent).not.toContain("sensitive backend failure");
  });

  it.each([
    ["stale", catalogState("loading")],
    ["missing", catalogState("ready", [])],
  ])("disables a database row for a %s descriptor and refreshes on interaction", (_case, state) => {
    mocks.catalogState = state;
    renderDatabase();
    const row = host.querySelector('[aria-disabled="true"]');

    expect(row).not.toBeNull();
    expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]?.disabled).toBe(true);
    act(() => row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));

    expect(projectHydration.refreshProjectResourceIndex).not.toHaveBeenCalled();
    expect(state.refresh).toHaveBeenCalledOnce();
  });

  it("disables a database row when its exact descriptor is missing", () => {
    const state = catalogState("ready", [
      item("Other database", {
        ...databaseSource,
        resourcePath: "databases/other",
      }),
    ]);
    mocks.catalogState = state;
    renderDatabase();
    const row = host.querySelector('[aria-disabled="true"]');

    expect(row).not.toBeNull();
    act(() => row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));
    expect(projectHydration.refreshProjectResourceIndex).not.toHaveBeenCalled();
    expect(state.refresh).toHaveBeenCalledOnce();
  });

  it("suppresses Catalog refresh when missing-path ProjectIndex hydration becomes stale", async () => {
    const state = catalogState("ready", []);
    mocks.catalogState = state;
    const refreshResourceIndex = vi.fn().mockResolvedValue(false);
    vi.spyOn(projectHydration, "refreshProjectResourceIndex").mockImplementation(
      refreshResourceIndex,
    );
    renderDatabase(null);
    const row = host.querySelector('[aria-disabled="true"]');

    await act(async () => {
      row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      await Promise.resolve();
    });

    expect(refreshResourceIndex).toHaveBeenCalledOnce();
    expect(state.refresh).not.toHaveBeenCalled();
  });

  it("hydrates a missing database path through ProjectIndex before refreshing Catalog and dragging", async () => {
    const state = catalogState("loading", []);
    mocks.catalogState = state;
    const refreshResourceIndex = vi.fn(async () => {
      useDatabaseStore.setState({
        databases: {
          "database-id": { id: "database-id", name: "Sales", resourcePath: databasePath },
        },
      });
      return true;
    });
    vi.spyOn(projectHydration, "refreshProjectResourceIndex").mockImplementation(
      refreshResourceIndex,
    );
    renderDatabase(null);
    const row = host.querySelector('[aria-disabled="true"]');

    await act(async () => {
      row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true }));
      await Promise.resolve();
    });

    expect(refreshResourceIndex).toHaveBeenCalledOnce();
    expect(state.refresh).toHaveBeenCalledOnce();
    mocks.catalogState = catalogState("ready", [item("Sales", databaseSource)]);
    renderDatabase(useDatabaseStore.getState().databases["database-id"]?.resourcePath);
    expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]).toMatchObject({
      disabled: false,
      data: { type: "node-template", template: { title: "Sales", descriptor: databaseSource } },
    });
  });
});
