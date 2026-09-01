import { beforeEach, describe, expect, it } from "vitest";
import { useNodeCatalogTreeStore } from "./nodeCatalogTreeStore";

describe("node catalog tree store", () => {
  beforeEach(() => {
    useNodeCatalogTreeStore.getState().reset();
  });

  it("stores only manual category expansion and query state", () => {
    const store = useNodeCatalogTreeStore.getState();

    store.setQuery("regression");
    store.setCategoryExpanded("statistics", true);

    expect(useNodeCatalogTreeStore.getState()).toMatchObject({
      query: "regression",
      expandedCategoryIds: new Set(["statistics"]),
    });
  });

  it("can replace stale expansion state when the Catalog scope changes", () => {
    const store = useNodeCatalogTreeStore.getState();

    store.setScope("project-1:zh-CN");
    store.setCategoryExpanded("statistics", true);
    store.setQuery("logit");
    store.setScope("project-2:en-US");

    expect(useNodeCatalogTreeStore.getState()).toMatchObject({
      scopeKey: "project-2:en-US",
      query: "",
      expandedCategoryIds: new Set(),
    });
  });

  it("can expand and collapse a set of categories together", () => {
    const store = useNodeCatalogTreeStore.getState();

    store.setCategoriesExpanded(["statistics", "math"], true);
    expect(useNodeCatalogTreeStore.getState().expandedCategoryIds).toEqual(
      new Set(["statistics", "math"]),
    );

    store.setCategoriesExpanded(["statistics", "math"], false);
    expect(useNodeCatalogTreeStore.getState().expandedCategoryIds).toEqual(new Set());
  });
});
