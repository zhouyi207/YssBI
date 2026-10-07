import { describe, expect, it, vi } from "vitest";
import type { NodeCreationDescriptor } from "@/features/domain/nodeCatalog/creationDescriptor";
import { findResourceNodeSpawnTemplate, spawnNodeFromTemplate } from "./spawnFromTemplate";

describe("spawnNodeFromTemplate", () => {
  it.each([
    {
      label: "static",
      descriptor: { kind: "static", nodeTypeId: "math.add" },
    },
    {
      label: "function_graph",
      descriptor: {
        kind: "resourceBound",
        nodeTypeId: "yssbi.project.function.call",
        resourcePath: "functions/Helper.yssbi-function",
        resourceRevision: 4,
        createArgs: { kind: "function_graph" },
      },
    },
    {
      label: "variable",
      descriptor: {
        kind: "resourceBound",
        nodeTypeId: "yssbi.project.database.get",
        resourcePath: "databases/00000000-0000-0000-0000-000000000001",
        resourceRevision: 5,
        createArgs: { kind: "database" },
      },
    },
    {
      label: "database",
      descriptor: {
        kind: "resourceBound",
        nodeTypeId: "yssbi.dataframe.source.get",
        resourcePath: "databases/sales / . # 数据",
        resourceRevision: 6,
        createArgs: { kind: "database" },
      },
    },
  ] satisfies Array<{ label: string; descriptor: NodeCreationDescriptor }>)(
    "forwards the exact $label descriptor without reconstruction",
    async ({ descriptor }) => {
      const createNode = vi.fn(async () => true);

      await expect(
        spawnNodeFromTemplate({ title: "Spawn", descriptor }, { x: 10, y: 20 }, { createNode }),
      ).resolves.toBe(true);

      expect(createNode).toHaveBeenCalledOnce();
      expect(createNode).toHaveBeenCalledWith(descriptor, { x: 10, y: 20 });
    },
  );

  it("looks up only the exact current opaque resource path and descriptor kind", () => {
    const descriptor: NodeCreationDescriptor = {
      kind: "resourceBound",
      nodeTypeId: "yssbi.project.function.call",
      resourcePath: "functions/opaque / . # 数据",
      resourceRevision: 9,
      createArgs: { kind: "function_graph" },
    };
    const items = [
      {
        nodeTypeId: "yssbi.project.function.call",
        title: "Opaque",
        available: true,
        documentation: null,
        categoryId: "functions",
        iconId: "function_graph",
        styleId: "call",
        aliases: [],
        technicalTerms: [],
        backendSearchText: ["opaque"],
        resourceNames: ["Opaque"],
        ports: [],
        parameters: [],
        resourcePath: descriptor.resourcePath,
        resourceRevision: 9,
        creation: descriptor,
      },
    ];

    expect(
      findResourceNodeSpawnTemplate(
        items,
        descriptor.resourcePath,
        "function_graph",
        "yssbi.project.function.call",
      ),
    ).toEqual({ title: "Opaque", descriptor });
    expect(
      findResourceNodeSpawnTemplate(
        items,
        descriptor.resourcePath,
        "function_graph",
        "yssbi.project.database.get",
      ),
    ).toBeNull();
    expect(findResourceNodeSpawnTemplate(items, "functions/opaque", "function_graph")).toBeNull();
    expect(findResourceNodeSpawnTemplate(items, descriptor.resourcePath, "database")).toBeNull();
    items[0].available = false;
    expect(
      findResourceNodeSpawnTemplate(items, descriptor.resourcePath, "function_graph"),
    ).toBeNull();

    const replacement = {
      ...items[0],
      available: true,
      title: "Updated",
      resourceRevision: 10,
      creation: { ...descriptor, resourceRevision: 10 },
    };
    expect(
      findResourceNodeSpawnTemplate([replacement], descriptor.resourcePath, "function_graph"),
    ).toEqual({ title: "Updated", descriptor: replacement.creation });
    expect(
      findResourceNodeSpawnTemplate(items, descriptor.resourcePath, "function_graph"),
    ).toBeNull();
  });
});
