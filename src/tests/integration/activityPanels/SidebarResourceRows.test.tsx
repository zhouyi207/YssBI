// @vitest-environment happy-dom
import { act } from "react";
import { useActivityPanelExpansion } from "@/features/application/sidebar/useActivityPanelExpansion";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LocalizedNodeCatalogState } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import { SidebarDataRow } from "@/modules/project-explorer/internal/ui/activity/SidebarDataRow";
import { SidebarProjectTab } from "@/modules/project-explorer/internal/ui/activity/SidebarProjectTab";
import type { ActivityPanelDocument } from "@/shared/types/domain/activityPanel";
import { activityPanelFixture, categoryFixture } from "@/tests/helpers/activityPanelFixture";
import { useSidebarStore, PROJECT_TREE_CATEGORY_IDS } from "@/features/core/sidebar";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  catalogState: null as LocalizedNodeCatalogState | null,
  document: null as ActivityPanelDocument | null,
  draggableInputs: [] as Array<{ data: unknown; disabled?: boolean }>,
  dragPointerDown: vi.fn(),
  openDatabase: vi.fn(),
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
vi.mock("@/features/application/sidebar/useProjectActivityPanelDocument", () => ({
  useProjectActivityPanelDocument: () => ({
    document: mocks.document,
    error: null,
    refresh: vi.fn(),
    ...useActivityPanelExpansion("project"),
  }),
}));
vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => mocks.catalogState,
}));
vi.mock("@/features/application/editor/openDatabaseInEditor", () => ({
  openDatabaseInEditor: mocks.openDatabase,
}));
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
    mocks.openDatabase.mockResolvedValue(undefined);
    useDatabaseStore.getState().clear();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderDatabase(loadFailed = false) {
    useDatabaseStore.getState().addDatabase("database-id", {
      id: "database-id",
      name: "Sales",
      resourcePath: databasePath,
      loadFailed,
    });
    act(() =>
      root.render(
        <SidebarDataRow
          id="database-id"
          resourcePath={databasePath}
          name="Sales"
          onContextMenu={vi.fn()}
        />,
      ),
    );
  }

  it("shows live data in the Project tree with category expansion and no search input", () => {
    useSidebarStore.setState({ expandedCategories: {} });
    mocks.document = activityPanelFixture("project", [
      ...Object.values(PROJECT_TREE_CATEGORY_IDS).map((id) => categoryFixture(id, id, 0, true)),
      {
        kind: "item",
        id: "database:database-id",
        depth: 1,
        item: { kind: "database", id: "database-id", name: "Sales", resourcePath: databasePath },
      },
    ]);
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
      useSidebarStore.getState().expandedCategories.project?.[PROJECT_TREE_CATEGORY_IDS.data],
    ).toBe(true);

    act(() => useDatabaseStore.getState().updateDatabase("database-id", { name: "Sales updated" }));
    expect(host.textContent).not.toContain("Sales updated");
    expect(host.textContent).toContain("Sales");
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

  it("opens data in the workbench on click", async () => {
    renderDatabase();
    await act(async () => {
      host.firstElementChild?.dispatchEvent(new MouseEvent("click", { bubbles: true }));
      await Promise.resolve();
    });

    expect(mocks.openDatabase).toHaveBeenCalledWith("database-id");
  });

  it("shows the localized load failure tooltip from machine state", () => {
    renderDatabase(true);

    expect(host.textContent).toContain("sidebar.dataLoadFailed");
  });

  it.each([
    ["stale", catalogState("loading")],
    ["missing", catalogState("ready", [])],
  ])(
    "keeps data openable for a %s descriptor and refreshes drag metadata on interaction",
    (_case, state) => {
      mocks.catalogState = state;
      renderDatabase();
      const row = host.firstElementChild as HTMLElement;

      expect(row.getAttribute("aria-disabled")).not.toBe("true");
      expect(row.style.opacity).toBe("");
      expect(row.classList.contains("cursor-pointer")).toBe(true);
      expect(mocks.draggableInputs[mocks.draggableInputs.length - 1]?.disabled).toBe(true);
      act(() => row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));

      expect(state.refresh).toHaveBeenCalledOnce();
    },
  );

  it("preserves data-row appearance when its exact drag descriptor is missing", () => {
    const state = catalogState("ready", [
      item("Other database", {
        ...databaseSource,
        resourcePath: "databases/other",
      }),
    ]);
    mocks.catalogState = state;
    renderDatabase();
    const row = host.firstElementChild as HTMLElement;

    expect(row.getAttribute("aria-disabled")).not.toBe("true");
    expect(row.style.opacity).toBe("");
    expect(row.classList.contains("cursor-pointer")).toBe(true);
    act(() => row!.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true })));
    expect(state.refresh).toHaveBeenCalledOnce();
  });
});
