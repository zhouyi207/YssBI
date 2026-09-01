// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getLocalizedSearchIndex } from "@/features/core/nodeCatalog/localizedSearchIndex";
import type { LocalizedNodeCatalogState } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { TooltipProvider } from "@/components/ui/tooltip";

const historyAvailability = vi.hoisted(() => ({
  activeTabId: null as string | null,
  canUndo: false,
  canRedo: false,
  pending: false,
}));

const draggableInputs = vi.hoisted(() => [] as Array<{ data: unknown; disabled?: boolean }>);

const catalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));

function readyCatalogState(): LocalizedNodeCatalogState {
  const catalog = {
    projectInstanceId: "project-1",
    registryFingerprint: "registry-1",
    resourcePublicationRevision: 1,
    locale: "en-US",
    categories: [
      { categoryId: "math", parentCategoryId: null, order: 10, title: "Math", searchText: "math" },
    ],
    items: [
      {
        nodeTypeId: "yssbi.numeric.add.int64",
        title: "Add",
        documentation: null,
        categoryId: "math",
        iconId: "math",
        styleId: "default",
        aliases: [],
        technicalTerms: [],
        backendSearchText: ["add"],
        resourceNames: [],
        ports: [],
        parameters: [],
        creation: { kind: "static" as const, nodeTypeId: "yssbi.numeric.add.int64" },
      },
    ],
  };
  return {
    status: "ready",
    error: null,
    catalog,
    searchIndex: getLocalizedSearchIndex(catalog),
    refresh: vi.fn(),
  };
}

function setInputValue(input: HTMLInputElement, value: string): void {
  const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
  setter?.call(input, value);
  input.dispatchEvent(new Event("input", { bubbles: true }));
}

vi.mock("@dnd-kit/core", () => ({
  useDraggable: (input: { data: unknown; disabled?: boolean }) => {
    draggableInputs.push(input);
    return { attributes: {}, listeners: {}, setNodeRef: vi.fn() };
  },
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

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "common.loading": "Loading...",
        "common.error": "Error",
        "common.incidentId": "Incident ID",
        "nodeCatalog.loadError": "Node catalog unavailable",
        "sidebar.noActiveGraph": "No active graph open",
        "sidebar.noActiveGraphDescription": "Open a graph to view commands",
        "common.undo": "Undo",
        "common.redo": "Redo",
      })[key] ?? key,
  }),
}));

vi.mock("@/features/application/editor", () => ({
  useEditorHistoryAvailability: () => historyAvailability,
}));

vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => catalogState.current,
}));

import { SidebarCommandsTab } from "./SidebarCommandsTab";
import { SidebarNodesTab } from "./SidebarNodesTab";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("Sidebar tab-level empty states", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    draggableInputs.length = 0;
    catalogState.current = readyCatalogState();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders all Catalog items as draggable templates", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );

    const input = host.querySelector("input");
    expect(input).not.toBeNull();
    act(() => setInputValue(input!, "add"));

    expect(host.textContent).toContain("Add");
    expect(host.textContent).not.toContain("yssbi.numeric.add.int64");
    expect(host.querySelector('[title="yssbi.numeric.add.int64"]')).not.toBeNull();

    expect(draggableInputs).toContainEqual({
      id: "sidebar-item-node-static:yssbi.numeric.add.int64",
      disabled: false,
      data: {
        type: "node-template",
        template: {
          title: "Add",
          descriptor: { kind: "static", nodeTypeId: "yssbi.numeric.add.int64" },
        },
      },
    });
    const dragData = draggableInputs[0]?.data as { template?: { descriptor?: unknown } };
    expect(dragData.template?.descriptor).toBe(catalogState.current?.catalog?.items[0].creation);
  });

  it("renders localized generic Catalog text, code, and incident ID", () => {
    catalogState.current = {
      status: "error",
      error: {
        code: "catalog_backend_failed",
        incidentId: "incident-sidebar-catalog-42",
      },
      catalog: null,
      searchIndex: null,
      refresh: vi.fn(),
    };

    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );

    expect(host.textContent).toContain("Node catalog unavailable");
    expect(host.textContent).toContain("[catalog_backend_failed]");
    expect(host.textContent).toContain("Incident ID: incident-sidebar-catalog-42");
  });

  it("uses the shared empty state when Commands has no active graph", () => {
    historyAvailability.activeTabId = null;
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain("No active graph open");
    expect(host.textContent).toContain("Open a graph to view commands");
    expect(host.querySelector('[data-slot="scroll-area-viewport"]')).toBeNull();
  });

  it("keeps command controls when an active graph exists", () => {
    historyAvailability.activeTabId = "events/Main.yssbi-event";
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain("Undo");
    expect(host.textContent).toContain("Redo");
  });
});
