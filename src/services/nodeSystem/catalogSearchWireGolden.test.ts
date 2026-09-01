import { describe, expect, it } from "vitest";
import catalogSearchWire from "@/tests/fixtures/node-system-contracts/catalog-search-wire.json";
import {
  buildCatalogSearchDocument,
  matchesCatalogSearchDocument,
} from "@/features/domain/nodeCatalog/searchDocument";
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from "@/shared/types/dto/localizedCatalog";

function parsedCatalog(): LocalizedCatalogDto {
  const wire: unknown = catalogSearchWire;
  expect(isLocalizedCatalogDto(wire)).toBe(true);
  if (!isLocalizedCatalogDto(wire)) {
    throw new Error("focused Rust Catalog fixture must pass the production strict guard");
  }
  return wire;
}

describe("Task 17 focused Catalog search wire golden", () => {
  it("is an exact strict localized Catalog response containing real Rust item wires", () => {
    const catalog = parsedCatalog();

    expect(Object.keys(catalog).sort()).toEqual([
      "categories",
      "items",
      "locale",
      "projectInstanceId",
      "registryFingerprint",
      "resourcePublicationRevision",
    ]);
    expect(catalog.items).toHaveLength(2);
    for (const item of catalog.items) {
      expect(item.backendSearchText).toBeInstanceOf(Array);
      expect(item.resourceNames).toBeInstanceOf(Array);
    }
  });

  it.each(["backendSearchText", "resourceNames"])("requires item field %s", (field) => {
    const wire = structuredClone(catalogSearchWire) as unknown as {
      items: Array<Record<string, unknown>>;
    };
    delete wire.items[0][field];

    expect(isLocalizedCatalogDto(wire)).toBe(false);
  });

  it("preserves raw Rust metadata until the shared frontend builder normalizes it", () => {
    const catalog = parsedCatalog();
    const staticItem = catalog.items.find((item) => item.nodeTypeId === "yssbi.numeric.add.int64");
    const resourceItem = catalog.items.find(
      (item) => item.resourcePath === "functions/catalog-search-wire",
    );
    expect(staticItem).toBeDefined();
    expect(resourceItem).toBeDefined();

    expect(staticItem!.backendSearchText).toEqual(["Add", "plus", "sum", "+"]);
    expect(resourceItem!.resourceNames).toEqual(["Straße_Data Cafe\u0301 数据"]);
    expect(resourceItem!.technicalTerms).toContain("Maße_Value\u0301");
    expect(resourceItem!.technicalTerms).toContain("技术_Term");

    const document = buildCatalogSearchDocument(resourceItem!);
    expect(document).toMatchObject({
      nodeTypeId: "yssbi.project.function.call",
      localizedTitle: "straße data cafe 数据",
      backendSearchText: ["call", "invoke", "function"],
      resourceNames: ["straße data cafe 数据"],
    });
    expect(document.technicalTerms).toContain("maße value");
    expect(document.technicalTerms).toContain("技术 term");
    expect(document.pinyinFull).toContain("ji shu term");
    expect(document.pinyinInitials).toContain("js term");
    expect(matchesCatalogSearchDocument(document, "straße data cafe")).toBe(true);
    expect(matchesCatalogSearchDocument(document, "strasse data cafe")).toBe(false);
  });
});
