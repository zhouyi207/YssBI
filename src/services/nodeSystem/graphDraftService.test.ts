import { invoke } from "@tauri-apps/api/core";
import { describe, expect, it, vi } from "vitest";
import editorProjection from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import { GraphDraftService } from "./graphDraftService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const graphPath = "events/contract.yssbi-event";
const document = { nodes: {}, port_bindings: [], connections: {}, input_states: [] };
const projectionReplacement = { graphPath, projection: editorProjection };

describe("GraphDraftService", () => {
  it("compiles a draft into a content-addressed projection without saving it", async () => {
    const result = {
      sourceHash: "a".repeat(64),
      cacheHit: false,
      document,
      projection: editorProjection,
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      GraphDraftService.compile("project-a", graphPath, "en-US", document),
    ).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith("compile_graph_draft", {
      projectInstanceId: "project-a",
      graphPath,
      locale: "en-US",
      document,
    });
  });

  it("accepts a frontend draft intent and returns its projection request identity", async () => {
    const mutation = { type: "deleteNodes" as const, payload: { nodeIds: [] } };
    const operationId = "00000000-0000-0000-0000-000000000011";
    const result = {
      projectInstanceId: "project-a",
      graphSessionId: "graph-session-a",
      graphPath,
      acceptedRevision: 1,
      requestGeneration: 2,
      operationId,
      document,
      patch: { operations: [] },
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      GraphDraftService.transform(
        "project-a",
        "graph-session-a",
        graphPath,
        "en-US",
        1,
        2,
        operationId,
        document,
        mutation,
      ),
    ).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith("transform_graph_draft", {
      projectInstanceId: "project-a",
      locale: "en-US",
      projectionRequest: {
        graphSessionId: "graph-session-a",
        graphPath,
        acceptedRevision: 1,
        requestGeneration: 2,
        operationId,
      },
      document,
      mutation,
    });
  });

  it("saves a complete draft with overwrite semantics and no expected revision", async () => {
    const operationId = "00000000-0000-0000-0000-000000000010";
    const result = {
      projectInstanceId: "project-a",
      operationId,
      document,
      projectionReplacement,
      history: { canUndo: true, canRedo: false },
    };
    vi.mocked(invoke).mockResolvedValue(result);

    await expect(
      GraphDraftService.save("project-a", graphPath, "en-US", operationId, document),
    ).resolves.toEqual(result);
    expect(invoke).toHaveBeenCalledWith("save_project_graph", {
      projectInstanceId: "project-a",
      graphPath,
      locale: "en-US",
      operationId,
      document,
    });
  });
});
