import { describe, expect, it } from "vitest";
import catalogSearchWire from "@/tests/fixtures/node-system-contracts/catalog-search-wire.json";
import {
  buildCatalogSearchDocument,
  catalogSearchTerms,
  matchesCatalogSearchDocument,
} from "@/features/domain/nodeCatalog/searchDocument";
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from "@/shared/types/dto/localizedCatalog";

function parsedCatalog(): LocalizedCatalogDto {
  const wire: unknown = catalogSearchWire;
  if (!isLocalizedCatalogDto(wire)) {
    throw new Error("focused Rust Catalog fixture must pass the production strict guard");
  }
  return wire;
}

describe("Catalog search wire contract", () => {
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
    const staticItem = catalog.items.find((item) => item.nodeTypeId === "yssbi.numeric.add");
    const resourceItem = catalog.items.find(
      (item) => item.resourcePath === "functions/catalog-search-wire",
    );
    expect(staticItem).toBeDefined();
    expect(resourceItem).toBeDefined();

    expect(staticItem!.backendSearchText).toEqual(["Add", "plus", "sum", "series add", "+"]);
    expect(resourceItem!.resourceNames).toEqual(["Straße_Data Cafe\u0301 数据"]);
    expect(resourceItem!.technicalTerms).toContain("Maße_Value\u0301");
    expect(resourceItem!.technicalTerms).toContain("技术_Term");

    const document = buildCatalogSearchDocument(resourceItem!);
    for (const text of [
      "yssbi project function call",
      "straße data cafe 数据",
      "call",
      "invoke",
      "function",
      "maße value",
      "技术 term",
      "ji shu term",
      "js term",
    ]) {
      expect(document).toContain(text);
    }
    expect(matchesCatalogSearchDocument(document, catalogSearchTerms("straße data cafe"))).toBe(
      true,
    );
    expect(matchesCatalogSearchDocument(document, catalogSearchTerms("strasse data cafe"))).toBe(
      false,
    );
  });
});
