import { beforeEach, describe, expect, it } from "vitest";
import { useNodeCatalogTreeStore } from "./nodeCatalogTreeStore";

describe("node catalog tree store", () => {
  beforeEach(() => {
    useNodeCatalogTreeStore.getState().reset();
  });

  it("stores manual category expansion", () => {
    const store = useNodeCatalogTreeStore.getState();

    store.setCategoryExpanded("statistics", true);

    expect(useNodeCatalogTreeStore.getState()).toMatchObject({
      expandedCategoryIds: new Set(["statistics"]),
    });
    store.setCategoryExpanded("statistics", false);
    expect(useNodeCatalogTreeStore.getState().expandedCategoryIds).toEqual(new Set());
  });

  it("can replace stale expansion state when the Catalog scope changes", () => {
    const store = useNodeCatalogTreeStore.getState();

    store.setScope("project-1:zh-CN");
    store.setCategoryExpanded("statistics", true);
    store.setScope("project-2:en-US");

    expect(useNodeCatalogTreeStore.getState()).toMatchObject({
      scopeKey: "project-2:en-US",
      expandedCategoryIds: new Set(),
    });
  });
});
