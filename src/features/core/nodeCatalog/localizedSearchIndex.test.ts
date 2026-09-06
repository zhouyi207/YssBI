import { describe, expect, it } from "vitest";
import { getLocalizedSearchIndex } from "./localizedSearchIndex";
import type { LocalizedCatalogResponse } from "./nodeCatalogStore";

function catalog(
  overrides: Partial<
    Pick<
      LocalizedCatalogResponse,
      "projectInstanceId" | "locale" | "registryFingerprint" | "resourcePublicationRevision"
    >
  > = {},
): LocalizedCatalogResponse {
  const nodeTypeId = [
    overrides.projectInstanceId ?? "project-1",
    overrides.locale ?? "zh-CN",
    overrides.registryFingerprint ?? "registry-1",
    overrides.resourcePublicationRevision ?? 7,
  ].join("-");
  return {
    projectInstanceId: "project-1",
    locale: "zh-CN",
    registryFingerprint: "registry-1",
    resourcePublicationRevision: 7,
    categories: [],
    items: [
      {
        nodeTypeId,
        title: nodeTypeId,
        documentation: null,
        categoryId: "tests",
        iconId: "tests",
        styleId: "default",
        aliases: [],
        technicalTerms: [],
        ports: [],
        parameters: [],
        creation: { kind: "static", nodeTypeId },
        backendSearchText: [],
        resourceNames: [],
      },
    ],
    ...overrides,
  };
}

describe("getLocalizedSearchIndex", () => {
  it("reuses an index only for the same response object", () => {
    const response = catalog();

    expect(getLocalizedSearchIndex(response)).toBe(getLocalizedSearchIndex(response));
  });

  it("preserves provenance for distinct equal-metadata responses", () => {
    const first = catalog();
    const second = catalog();
    first.items[0].title = "First response title";
    second.items[0].title = "第二响应";

    const firstIndex = getLocalizedSearchIndex(first);
    const secondIndex = getLocalizedSearchIndex(second);

    expect(secondIndex).not.toBe(firstIndex);
    expect(firstIndex.response).toBe(first);
    expect(secondIndex.response).toBe(second);
    expect(firstIndex.search("first response")[0]).toBe(first.items[0]);
    expect(secondIndex.search("di er xiang ying")[0]).toBe(second.items[0]);
    expect(secondIndex.search("first response")).toEqual([]);
  });

  it("indexes the shared search document while preserving the original item", () => {
    const response = catalog({ resourcePublicationRevision: 99 });
    const item = response.items[0];
    item.title = "当前标题";
    item.aliases = ["current alias"];
    item.technicalTerms = ["technical-only-secret", "技术术语"];
    item.backendSearchText = ["backend-search-only-secret"];
    item.resourceNames = ["季度销售"];

    const index = getLocalizedSearchIndex(response);

    for (const query of [
      "当前",
      "current alias",
      "technical-only-secret",
      "ji shu shu yu",
      "jssy",
      item.nodeTypeId,
      "backend-search-only-secret",
      "季度销售",
      "dang qian biao ti",
      "dqbt",
    ]) {
      expect(index.search(query)).toEqual([item]);
    }
    expect(index.response.items[0]).toBe(item);
  });
});
