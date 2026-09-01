import { describe, expect, it } from "vitest";
import type {
  LocalizedCatalogItemDto,
  LocalizedCategoryDto,
} from "@/shared/types/dto/localizedCatalog";
import { catalogItemKey } from "./catalogItem";
import {
  buildLocalizedCatalogTree,
  collectLocalizedCatalogCategoryIds,
  flattenLocalizedCatalogTree,
} from "./localizedCatalogTree";

const categories: LocalizedCategoryDto[] = [
  {
    categoryId: "statistics.regression",
    parentCategoryId: "statistics",
    order: 11,
    title: "Regression",
    searchText: "regression",
  },
  {
    categoryId: "output",
    parentCategoryId: null,
    order: 20,
    title: "Output",
    searchText: "output",
  },
  {
    categoryId: "statistics",
    parentCategoryId: null,
    order: 10,
    title: "Statistics",
    searchText: "statistics",
  },
];

function item(
  nodeTypeId: string,
  categoryId: string,
  creation: LocalizedCatalogItemDto["creation"] = { kind: "static", nodeTypeId },
): LocalizedCatalogItemDto {
  return {
    nodeTypeId,
    title: nodeTypeId,
    documentation: null,
    categoryId,
    iconId: "test",
    styleId: "default",
    aliases: [],
    technicalTerms: [],
    backendSearchText: [],
    resourceNames: [],
    ports: [],
    parameters: [],
    creation,
  };
}

describe("buildLocalizedCatalogTree", () => {
  it("builds an ordered hierarchy independently of category input order", () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item("output.print", "output"),
      item("statistics.logit.fit", "statistics.regression"),
    ]);

    expect(tree.map((node) => node.category.categoryId)).toEqual(["statistics", "output"]);
    expect(tree[0].children.map((node) => node.category.categoryId)).toEqual([
      "statistics.regression",
    ]);
    expect(tree[0].children[0].items.map((entry) => entry.nodeTypeId)).toEqual([
      "statistics.logit.fit",
    ]);
  });

  it("omits empty branches while retaining ancestors of matching items", () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item("statistics.logit.fit", "statistics.regression"),
    ]);

    expect(tree).toHaveLength(1);
    expect(tree[0].category.categoryId).toBe("statistics");
    expect(tree[0].children[0].category.categoryId).toBe("statistics.regression");
  });

  it("uses resource paths as part of resource-bound item identity", () => {
    const first = item("function.call", "output", {
      kind: "resourceBound",
      nodeTypeId: "function.call",
      resourcePath: "functions/First",
      resourceRevision: 1,
      createArgs: { kind: "function" },
    });
    const second = item("function.call", "output", {
      kind: "resourceBound",
      nodeTypeId: "function.call",
      resourcePath: "functions/Second",
      resourceRevision: 2,
      createArgs: { kind: "function" },
    });
    const refreshed = item("function.call", "output", {
      kind: "resourceBound",
      nodeTypeId: "function.call",
      resourcePath: "functions/First",
      resourceRevision: 3,
      createArgs: { kind: "function" },
    });

    expect(catalogItemKey(first)).not.toBe(catalogItemKey(second));
    expect(catalogItemKey(first)).toBe(catalogItemKey(refreshed));
  });

  it("uses the stable item identity as a deterministic title tie-breaker", () => {
    const second = item("function.call", "output", {
      kind: "resourceBound",
      nodeTypeId: "function.call",
      resourcePath: "functions/Second",
      resourceRevision: 1,
      createArgs: { kind: "function" },
    });
    const first = {
      ...item("function.call", "output", {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/First",
        resourceRevision: 1,
        createArgs: { kind: "function" },
      }),
      title: second.title,
    };
    const tree = buildLocalizedCatalogTree(categories, [second, first]);

    expect(
      tree.find((node) => node.category.categoryId === "output")?.items.map(catalogItemKey),
    ).toEqual([catalogItemKey(first), catalogItemKey(second)]);
  });

  it("flattens only expanded categories in the same item-before-child order as Canvas", () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item("output.print", "output"),
      item("statistics.logit.fit", "statistics.regression"),
    ]);

    const rows = flattenLocalizedCatalogTree(
      tree,
      new Set(["statistics", "statistics.regression", "output"]),
    );

    expect(
      rows.map(
        (row) =>
          `${row.kind}:${row.kind === "category" ? row.category.categoryId : row.item.nodeTypeId}`,
      ),
    ).toEqual([
      "category:statistics",
      "category:statistics.regression",
      "item:statistics.logit.fit",
      "category:output",
      "item:output.print",
    ]);
    expect(rows[1]).toMatchObject({ kind: "category", depth: 1 });
  });

  it("returns all populated ancestors for search-driven expansion", () => {
    const tree = buildLocalizedCatalogTree(categories, [
      item("statistics.logit.fit", "statistics.regression"),
    ]);

    expect(collectLocalizedCatalogCategoryIds(tree)).toEqual(
      new Set(["statistics", "statistics.regression"]),
    );
  });
});
