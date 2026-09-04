// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { LocalizedNodeCatalogState } from "@/features/application/nodeCatalog/useLocalizedNodeCatalog";
import { getLocalizedSearchIndex } from "@/features/core/nodeCatalog/localizedSearchIndex";
import type { LocalizedCatalogResponse } from "@/features/core/nodeCatalog/nodeCatalogStore";
import { NodeDocumentationModal } from "./NodeDocumentationModal";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const katexWarningSpy = vi.hoisted(() => {
  const warn = console.warn.bind(console);
  return vi.spyOn(console, "warn").mockImplementation((message, ...args) => {
    const quirksWarning =
      "Warning: KaTeX doesn't work in quirks mode. Make sure your website has a suitable doctype.";
    if (message !== quirksWarning) warn(message, ...args);
  });
});

const catalogState = vi.hoisted(() => ({
  current: null as LocalizedNodeCatalogState | null,
}));

vi.mock("@/features/application/nodeCatalog/useLocalizedNodeCatalog", () => ({
  useLocalizedNodeCatalog: () => catalogState.current,
}));

const translations: Record<string, string> = {
  "common.error": "Error",
  "common.incidentId": "Incident ID",
  "common.loading": "Loading...",
  "nodeCatalog.loadError": "Node catalog unavailable",
  "nodeDocumentationModal.title": "Node Documentation",
  "nodeDocumentationModal.description": "Search current-language node titles and aliases.",
  "nodeDocumentationModal.searchPlaceholder": "Search node titles and aliases...",
  "nodeDocumentationModal.noMatches": "No matching node documentation",
  "nodeDocumentationModal.noDocumentation": "This node has no detailed documentation yet.",
  "nodeDocumentationModal.selectNode": "Select a node to view its documentation.",
  "nodeDocumentationModal.close": "Close node documentation",
  "nodeDocumentationModal.nodeId": "Node ID",
  "nodeDocumentationModal.ports": "Ports",
  "nodeDocumentationModal.noPorts": "No ports",
  "nodeDocumentationModal.parameters": "Parameters",
  "nodeDocumentationModal.noParameters": "No parameters",
  "nodeDocumentationModal.resourcePath": "Resource path",
  "nodeDocumentationModal.resourceRevision": "Resource revision",
};

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => translations[key] ?? key }),
}));

function catalog(
  locale: string,
  localized: {
    title: string;
    documentation: string;
    alias: string;
  },
): LocalizedCatalogResponse {
  const resourcePath = "functions/opaque/%E5%8A%A9%E6%89%8B.fn";
  return {
    projectInstanceId: "project-1",
    registryFingerprint: `registry-${locale}`,
    resourcePublicationRevision: 17,
    locale,
    categories: [
      {
        categoryId: "functions",
        parentCategoryId: null,
        order: 0,
        title: "Functions",
        searchText: "Functions",
      },
    ],
    items: [
      {
        nodeTypeId: "function.call",
        title: localized.title,
        documentation: localized.documentation,
        categoryId: "functions",
        iconId: "function",
        styleId: "call",
        aliases: [localized.alias],
        technicalTerms: [],
        backendSearchText: [localized.title, localized.alias],
        resourceNames: [localized.title],
        ports: [
          {
            key: "result",
            label: locale === "zh-CN" ? "结果" : "Result",
            direction: "output",
          },
        ],
        parameters: [
          {
            key: "timeout",
            title: locale === "zh-CN" ? "超时" : "Timeout",
            description: locale === "zh-CN" ? "最长等待时间" : "Maximum wait time",
          },
        ],
        resourcePath,
        resourceRevision: 9,
        creation: {
          kind: "resourceBound",
          nodeTypeId: "function.call",
          resourcePath,
          resourceRevision: 9,
          createArgs: { kind: "function" },
        },
      },
    ],
  };
}

function stateFor(response: LocalizedCatalogResponse | null): LocalizedNodeCatalogState {
  return {
    status: "ready",
    error: null,
    catalog: response,
    searchIndex: response ? getLocalizedSearchIndex(response) : null,
    refresh: vi.fn(),
  };
}

function click(element: Element): void {
  act(() => element.dispatchEvent(new MouseEvent("click", { bubbles: true })));
}

function input(element: HTMLInputElement, value: string): void {
  act(() => {
    const setter = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")?.set;
    setter?.call(element, value);
    element.dispatchEvent(new Event("input", { bubbles: true }));
  });
}

describe("NodeDocumentationModal", () => {
  let host: HTMLDivElement;
  let root: Root;
  const onOpenChange = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    document.body.innerHTML = "";
  });

  afterAll(() => katexWarningSpy.mockRestore());

  function render(): void {
    act(() => root.render(<NodeDocumentationModal open onOpenChange={onOpenChange} />));
  }

  it("renders current-locale documentation and focused Catalog metadata", () => {
    catalogState.current = stateFor(
      catalog("zh-CN", {
        title: "调用助手",
        documentation: "仅中文 **详细文档**",
        alias: "助手调用",
      }),
    );
    render();

    click(document.querySelector("button[data-node-documentation-item]")!);

    expect(document.body.textContent).toContain("调用助手");
    expect(document.body.textContent).toContain("仅中文 详细文档");
    expect(document.querySelector("strong")?.textContent).toBe("详细文档");
    expect(document.body.textContent).not.toContain("English documentation");
    expect(document.body.textContent).toContain("Node ID");
    expect(document.body.textContent).toContain("function.call");
    expect(document.body.textContent).toContain("result");
    expect(document.body.textContent).toContain("结果");
    expect(document.body.textContent).toContain("data");
    expect(document.body.textContent).toContain("timeout");
    expect(document.body.textContent).toContain("超时");
    expect(document.body.textContent).toContain("functions/opaque/%E5%8A%A9%E6%89%8B.fn");
    expect(document.body.textContent).toContain("9");
  });

  it("renders localized generic catalog text, code, and incident ID", () => {
    catalogState.current = {
      status: "error",
      error: {
        code: "catalog_backend_failed",
        incidentId: "incident-documentation-catalog-42",
      },
      catalog: null,
      searchIndex: null,
      refresh: vi.fn(),
    };

    render();

    expect(document.body.textContent).toContain("Node catalog unavailable");
    expect(document.body.textContent).toContain("[catalog_backend_failed]");
    expect(document.body.textContent).toContain("Incident ID: incident-documentation-catalog-42");
  });

  it("searches the current Catalog index without searching documentation bodies", () => {
    catalogState.current = stateFor(
      catalog("en-US", {
        title: "Call Helper",
        documentation: "documentation-only-secret",
        alias: "invoke helper",
      }),
    );
    render();
    const search = document.querySelector<HTMLInputElement>(
      'input[placeholder="Search node titles and aliases..."]',
    )!;

    input(search, "documentation-only-secret");
    expect(document.body.textContent).toContain("No matching node documentation");
    expect(document.querySelector("[data-node-documentation-item]")).toBeNull();

    input(search, "invoke helper");
    expect(document.body.textContent).toContain("Call Helper");
    expect(document.querySelector("[data-node-documentation-item]")).not.toBeNull();
  });

  it("limits documentation search to the current-locale title and aliases", () => {
    const response = catalog("en-US", {
      title: "Call Helper",
      documentation: "English documentation",
      alias: "invoke helper",
    });
    response.registryFingerprint = "registry-en-US-documentation-search";
    response.items[0].technicalTerms = ["technical-only-secret"];
    response.items[0].backendSearchText = ["backend-search-only-secret"];
    catalogState.current = stateFor(response);
    render();
    const search = document.querySelector<HTMLInputElement>(
      'input[placeholder="Search node titles and aliases..."]',
    )!;

    for (const excludedQuery of [
      "technical-only-secret",
      "function.call",
      "backend-search-only-secret",
      "pin yin only secret",
    ]) {
      input(search, excludedQuery);
      expect(document.querySelector("[data-node-documentation-item]")).toBeNull();
    }

    for (const includedQuery of ["Call Helper", "invoke helper"]) {
      input(search, includedQuery);
      expect(document.querySelector("[data-node-documentation-item]")).not.toBeNull();
    }
  });

  it("keeps the opaque resource identity selected across a locale switch", () => {
    catalogState.current = stateFor(
      catalog("en-US", {
        title: "Call Helper",
        documentation: "English documentation",
        alias: "invoke helper",
      }),
    );
    render();
    click(document.querySelector("button[data-node-documentation-item]")!);
    expect(document.body.textContent).toContain("English documentation");

    catalogState.current = stateFor(
      catalog("zh-CN", {
        title: "调用助手",
        documentation: "中文文档",
        alias: "助手调用",
      }),
    );
    render();

    expect(document.body.textContent).toContain("调用助手");
    expect(document.body.textContent).toContain("中文文档");
    expect(document.body.textContent).not.toContain("English documentation");
  });

  it("shows Catalog-derived empty state and supports preview and modal close behavior", () => {
    catalogState.current = stateFor(
      catalog("en-US", {
        title: "Call Helper",
        documentation: "English documentation",
        alias: "invoke helper",
      }),
    );
    render();
    const item = document.querySelector("button[data-node-documentation-item]")!;

    click(item);
    expect(document.body.textContent).toContain("English documentation");
    click(item);
    expect(document.body.textContent).not.toContain("English documentation");

    click(document.querySelector('button[aria-label="Close node documentation"]')!);
    expect(onOpenChange).toHaveBeenCalledWith(false);

    catalogState.current = stateFor({
      ...catalog("en-US", {
        title: "unused",
        documentation: "unused",
        alias: "unused",
      }),
      registryFingerprint: "registry-en-US-empty",
      items: [],
    });
    render();
    expect(document.body.textContent).toContain("No matching node documentation");
  });
});
