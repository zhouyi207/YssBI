import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import localizedCatalog from "@/tests/fixtures/node-system-contracts/localized-catalog.json";
import editorProjection from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import type { LocalizedCatalogDto } from "./catalogService";
import { CatalogService } from "./catalogService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const version = { sessionId: "00000000-0000-0000-0000-000000000090", revision: "0" };

describe("CatalogService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("queries a creation form without a node identity and rejects malformed parameter or pin contracts", async () => {
    const response = {
      values: { columns: [" sales "] },
      groups: editorProjection.nodes.flatMap((node) => node.parameterGroups),
      portCounts: { x: 4 },
      ports: [
        {
          key: "x",
          title: "X",
          direction: "input",
          count: { kind: "configurable", min: 0, max: null, memberTemplates: ["x"] },
        },
      ],
    };
    // Reuse a single existing parameter group, whose keys are unique within a node.
    response.groups = response.groups.slice(0, 1);
    vi.mocked(invoke).mockResolvedValue(response);
    await expect(
      CatalogService.getNodeCreationForm("project", "node", response.values, { x: 4 }, "en-US"),
    ).resolves.toEqual(response);
    expect(invoke).toHaveBeenCalledWith("get_node_creation_form", {
      projectInstanceId: "project",
      nodeTypeId: "node",
      parameters: response.values,
      portCounts: { x: 4 },
      locale: "en-US",
    });
    for (const invalid of [
      { ...response, portCounts: { x: 1.5 } },
      { ...response, groups: [{ key: "missing-fields" }] },
    ]) {
      vi.mocked(invoke).mockResolvedValue(invalid);
      await expect(
        CatalogService.getNodeCreationForm("project", "node", {}, {}, "en-US"),
      ).rejects.toThrow();
    }
  });

  it("requests a backend-filtered compatible catalog for the current graph version", async () => {
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
        version,
        sourcePort,
        locale: localizedCatalog.locale,
      }),
    ).resolves.toBe(localizedCatalog);

    expect(invoke).toHaveBeenCalledWith("get_compatible_node_catalog", {
      projectInstanceId: localizedCatalog.projectInstanceId,
      graphPath: "events/Main.yssbi-event",
      version,
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
        version,
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

  it("rejects resource metadata that does not exactly match its descriptor", async () => {
    const catalog = structuredClone(localizedCatalog) as unknown as LocalizedCatalogDto;
    const resource = catalog.items.find((item) => item.creation.kind === "resourceBound");
    if (!resource) throw new Error("Rust fixture must include a resource-bound item");
    resource.resourcePath = "functions/Mismatched.yssbi-function";
    vi.mocked(invoke).mockResolvedValue(catalog);

    await expect(
      CatalogService.getLocalizedCatalog(catalog.projectInstanceId, catalog.locale),
    ).rejects.toThrow("Invalid localized node catalog response");
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
