// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { getLocalizedSearchIndex } from "@/features/core/nodeCatalog/localizedSearchIndex";
import type { LocalizedCatalogResponse } from "@/features/core/nodeCatalog/nodeCatalogStore";
import type { LocalizedNodeCatalogState } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { useNodeCatalogTreeStore } from "@/features/core/nodeCatalog/nodeCatalogTreeStore";
import { TooltipProvider } from "@/components/ui/tooltip";

const draggableInputs = vi.hoisted(() => [] as Array<{ id: string; data: unknown }>);
const catalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));

vi.mock("@dnd-kit/core", () => ({
  useDraggable: (input: { id: string; data: unknown }) => {
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

vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string) =>
      ({
        "common.loading": "Loading...",
        "nodeCatalog.loadError": "Node catalog unavailable",
        "sidebar.noNodes": "No nodes available",
        "activityBar.nodes": "Nodes",
      })[key] ?? key,
  }),
}));

vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => catalogState.current,
}));

import { SidebarNodesTab } from "./SidebarNodesTab";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const catalog: LocalizedCatalogResponse = {
  projectInstanceId: "project-1",
  registryFingerprint: "registry-1",
  resourcePublicationRevision: 1,
  locale: "en-US",
  categories: [
    {
      categoryId: "statistics.regression",
      parentCategoryId: "statistics",
      order: 11,
      title: "Regression",
      searchText: "regression",
    },
    {
      categoryId: "statistics",
      parentCategoryId: null,
      order: 10,
      title: "Statistics",
      searchText: "statistics",
    },
    {
      categoryId: "output",
      parentCategoryId: null,
      order: 20,
      title: "Output",
      searchText: "output",
    },
  ],
  items: [
    {
      nodeTypeId: "statistics.logit.fit",
      title: "Logit fit",
      documentation: null,
      categoryId: "statistics.regression",
      iconId: "statistics",
      styleId: "default",
      aliases: ["logit"],
      technicalTerms: [],
      backendSearchText: [],
      resourceNames: [],
      ports: [],
      parameters: [],
      creation: { kind: "static", nodeTypeId: "statistics.logit.fit" },
    },
    {
      nodeTypeId: "function.call",
      title: "Call Helper",
      documentation: null,
      categoryId: "output",
      iconId: "function",
      styleId: "default",
      aliases: [],
      technicalTerms: [],
      backendSearchText: [],
      resourceNames: ["Helper Resource"],
      ports: [],
      parameters: [],
      resourcePath: "functions/Helper",
      resourceRevision: 1,
      creation: {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/Helper",
        resourceRevision: 1,
        createArgs: { kind: "function" },
      },
    },
    {
      nodeTypeId: "function.call",
      title: "Call Other",
      documentation: null,
      categoryId: "output",
      iconId: "function",
      styleId: "default",
      aliases: [],
      technicalTerms: [],
      backendSearchText: [],
      resourceNames: ["Other Resource"],
      ports: [],
      parameters: [],
      resourcePath: "functions/Other",
      resourceRevision: 1,
      creation: {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/Other",
        resourceRevision: 1,
        createArgs: { kind: "function" },
      },
    },
  ],
};

function readyState(): LocalizedNodeCatalogState {
  return {
    status: "ready",
    error: null,
    catalog,
    searchIndex: getLocalizedSearchIndex(catalog),
    refresh: vi.fn(),
  };
}

describe("SidebarNodesTab", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useNodeCatalogTreeStore.getState().reset();
    draggableInputs.length = 0;
    catalogState.current = readyState();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders the catalog without search controls and keeps categories expandable", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );

    expect(host.querySelector("[data-sidebar-tree-search]")).toBeNull();
    expect(host.querySelector("input")).toBeNull();
    expect(host.textContent).toContain("Statistics");
    expect(host.textContent).not.toContain("Logit fit");

    const statistics = host.querySelector<HTMLButtonElement>(
      '[data-sidebar-tree-category-id="statistics"]',
    )!;
    act(() => statistics.click());
    const regression = host.querySelector<HTMLButtonElement>(
      '[data-sidebar-tree-category-id="statistics.regression"]',
    )!;
    act(() => regression.click());

    expect(host.textContent).toContain("Statistics");
    expect(host.textContent).toContain("Regression");
    expect(host.textContent).toContain("Logit fit");
    expect(host.textContent).not.toContain("statistics.logit.fit");
    expect(host.textContent).not.toContain("Call Helper");
    act(() => statistics.click());
    expect(host.textContent).not.toContain("Logit fit");
    act(() => statistics.click());
    expect(host.textContent).toContain("Logit fit");
  });

  it("keeps resource-bound entries distinct while preserving their descriptors", () => {
    act(() =>
      root.render(
        <TooltipProvider>
          <SidebarNodesTab />
        </TooltipProvider>,
      ),
    );
    const output = host.querySelector<HTMLButtonElement>(
      '[data-sidebar-tree-category-id="output"]',
    )!;
    act(() => output.click());

    const nodeInputs = draggableInputs.filter(
      ({ data }) => (data as { type?: string }).type === "node-template",
    );
    expect(nodeInputs).toHaveLength(2);
    expect(new Set(nodeInputs.map(({ id }) => id)).size).toBe(2);
    expect(
      nodeInputs.map(
        ({ data }) =>
          (
            data as {
              template: { descriptor: unknown };
            }
          ).template.descriptor,
      ),
    ).toEqual([catalog.items[1].creation, catalog.items[2].creation]);
  });
});
