import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import localizedCatalog from "@/tests/fixtures/node-system-contracts/localized-catalog.json";
import type { LocalizedCatalogDto } from "./catalogService";
import { CatalogService } from "./catalogService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const registryFingerprint = "0000000000000000000000000000000000000000000000000000000000000000";
const draftDocument = {
  nodes: {},
  port_bindings: [],
  connections: {},
  input_states: [],
};

describe("CatalogService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("requests a backend-filtered compatible catalog for the supplied Graph Draft", async () => {
    vi.mocked(invoke).mockResolvedValue(localizedCatalog);
    const sourcePort = {
      kind: "declared" as const,
      nodeId: "00000000-0000-0000-0000-000000000101",
      portKey: "value",
    };

    await expect(
      CatalogService.getCompatibleNodeCatalog({
        projectInstanceId: localizedCatalog.projectInstanceId,
        graphPath: "events/Main.yssbi-event",
        document: draftDocument,
        sourcePort,
        locale: localizedCatalog.locale,
      }),
    ).resolves.toBe(localizedCatalog);

    expect(invoke).toHaveBeenCalledWith("get_compatible_node_catalog", {
      projectInstanceId: localizedCatalog.projectInstanceId,
      graphPath: "events/Main.yssbi-event",
      document: draftDocument,
      sourcePort,
      locale: localizedCatalog.locale,
    });
  });

  it("rejects a malformed compatible catalog response", async () => {
    vi.mocked(invoke).mockResolvedValue({ ...localizedCatalog, extra: true });

    await expect(
      CatalogService.getCompatibleNodeCatalog({
        projectInstanceId: localizedCatalog.projectInstanceId,
        graphPath: "events/Main.yssbi-event",
        document: draftDocument,
        sourcePort: {
          kind: "declared",
          nodeId: "00000000-0000-0000-0000-000000000101",
          portKey: "value",
        },
        locale: localizedCatalog.locale,
      }),
    ).rejects.toThrow("Invalid compatible node catalog response");
  });

  it("accepts the authoritative Rust Catalog fixture through mocked invoke", async () => {
    vi.mocked(invoke).mockResolvedValue(localizedCatalog);

    await expect(
      CatalogService.getLocalizedCatalog(
        localizedCatalog.projectInstanceId,
        localizedCatalog.locale,
      ),
    ).resolves.toBe(localizedCatalog);
    expect(invoke).toHaveBeenCalledWith("get_localized_node_catalog", {
      projectInstanceId: localizedCatalog.projectInstanceId,
      locale: localizedCatalog.locale,
    });
  });

  it.each([
    ["sideways direction", "direction", "sideways"],
    ["execution kind", "kind", "execution"],
  ])("rejects Catalog port %s", async (_label, field, malformed) => {
    const catalog = structuredClone(localizedCatalog) as unknown as LocalizedCatalogDto;
    Object.assign(catalog.items[0].ports[0], { [field]: malformed });
    vi.mocked(invoke).mockResolvedValue(catalog);

    await expect(
      CatalogService.getLocalizedCatalog(catalog.projectInstanceId, catalog.locale),
    ).rejects.toThrow("Invalid localized node catalog response");
  });

  it("rejects resource metadata that does not exactly match its descriptor", async () => {
    vi.mocked(invoke).mockResolvedValue({
      projectInstanceId: "project-instance-1",
      registryFingerprint,
      resourcePublicationRevision: 8,
      locale: "en-US",
      categories: [],
      items: [
        {
          nodeTypeId: "function.call",
          title: "Call A",
          documentation: null,
          categoryId: "functions",
          iconId: "function",
          styleId: "call",
          aliases: [],
          technicalTerms: [],
          backendSearchText: [],
          resourceNames: [],
          ports: [],
          parameters: [],
          resourcePath: "functions/A",
          resourceRevision: 2,
          creation: {
            kind: "resourceBound",
            nodeTypeId: "function.call",
            resourcePath: "functions/B",
            resourceRevision: 2,
            createArgs: { kind: "function" },
          },
        },
      ],
    });

    await expect(CatalogService.getLocalizedCatalog("project-instance-1", "en-US")).rejects.toThrow(
      "Invalid localized node catalog response",
    );
  });

  it.each([
    ["missing static field", { kind: "static" }],
    ["extra static field", { kind: "static", nodeTypeId: "math.add", extra: true }],
    [
      "missing parameterized field",
      {
        kind: "parameterizedStatic",
        nodeTypeId: "yssbi.dataframe.project",
      },
    ],
    [
      "extra parameterized field",
      {
        kind: "parameterizedStatic",
        nodeTypeId: "yssbi.dataframe.project",
        requiredParameters: ["columns"],
        parameters: {},
      },
    ],
    [
      "wrong parameterized key list",
      {
        kind: "parameterizedStatic",
        nodeTypeId: "yssbi.dataframe.project",
        requiredParameters: "columns",
      },
    ],
    [
      "missing resource field",
      {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/A",
        resourceRevision: 1,
      },
    ],
    [
      "extra resource field",
      {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/A",
        resourceRevision: 1,
        createArgs: { kind: "function" },
        extra: true,
      },
    ],
    [
      "extra create args field",
      {
        kind: "resourceBound",
        nodeTypeId: "function.call",
        resourcePath: "functions/A",
        resourceRevision: 1,
        createArgs: { kind: "function", extra: true },
      },
    ],
  ])("rejects a descriptor with %s", async (_label, creation) => {
    vi.mocked(invoke).mockResolvedValue({
      projectInstanceId: "project-instance-1",
      registryFingerprint,
      resourcePublicationRevision: 8,
      locale: "en-US",
      categories: [],
      items: [
        {
          nodeTypeId: "function.call",
          title: "Call A",
          documentation: null,
          categoryId: "functions",
          iconId: "function",
          styleId: "call",
          aliases: [],
          technicalTerms: [],
          backendSearchText: [],
          resourceNames: [],
          ports: [],
          parameters: [],
          creation,
        },
      ],
    });

    await expect(CatalogService.getLocalizedCatalog("project-instance-1", "en-US")).rejects.toThrow(
      "Invalid localized node catalog response",
    );
  });

  it("rejects an unknown item field", async () => {
    const catalog = structuredClone(localizedCatalog) as Record<string, unknown>;
    const items = catalog.items as Record<string, unknown>[];
    items[0].unexpectedField = true;
    vi.mocked(invoke).mockResolvedValue(catalog);

    await expect(
      CatalogService.getLocalizedCatalog(
        catalog.projectInstanceId as string,
        catalog.locale as string,
      ),
    ).rejects.toThrow("Invalid localized node catalog response");
  });
});
