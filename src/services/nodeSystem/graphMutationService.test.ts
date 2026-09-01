import { invoke } from "@tauri-apps/api/core";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import type { EditorGraphMutationDto, MutationRequestDto } from "@/shared/types/dto/editorMutation";
import { GraphMutationService } from "./graphMutationService";

vi.mock("@tauri-apps/api/core", () => ({ invoke: vi.fn() }));

const projectInstanceId = "00000000-0000-0000-0000-000000000601";
const graphPath = "functions/Main.yssbi-function";
const operationId = "00000000-0000-0000-0000-000000000602";

const request: MutationRequestDto<EditorGraphMutationDto> = {
  resource: { kind: "graph", key: graphPath },
  baseRevision: 1,
  operationId,
  payload: { type: "deleteNodes", payload: { nodeIds: ["local-node"] } },
};

function graphMutationWireResult(): unknown {
  return {
    projectInstanceId,
    delta: {
      graphPath,
      fromRevision: 1,
      toRevision: 2,
      causedBy: operationId,
      payload: {
        operations: [
          {
            operation: "remove_node",
            node: {
              id: "00000000-0000-0000-0000-000000000604",
              node_type: "tests.node",
              position: { x: 0, y: 0 },
              parameters: {},
              user_label: null,
            },
          },
        ],
      },
    },
    projectionReplacement: {
      graphPath,
      projection: makeEditorProjectionFixture({
        graphPath,
        sourceRevision: 2,
        nodeId: "00000000-0000-0000-0000-000000000603",
        title: "Committed",
      }).projection,
      functionEditorProjection: {
        functionRevision: 2,
        inputs: [],
        outputs: [],
      },
    },
    history: { canUndo: true, canRedo: false },
  };
}

describe("GraphMutationService", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it.each<EditorGraphMutationDto>([
    { type: "deleteNodes", payload: { nodeIds: ["node-a", "node-b"] } },
    { type: "disconnectConnections", payload: { connectionIds: ["connection-a"] } },
    {
      type: "disconnectPort",
      payload: { address: { kind: "declared", nodeId: "node-a", portKey: "output" } },
    },
    { type: "disconnectNode", payload: { nodeId: "node-a" } },
    {
      type: "moveConnections",
      payload: {
        source: { kind: "declared", nodeId: "node-a", portKey: "output" },
        target: { kind: "declared", nodeId: "node-b", portKey: "output" },
      },
    },
  ])("forwards the $type intent in one mutate_graph_document invoke", async (payload) => {
    const response = graphMutationWireResult();
    vi.mocked(invoke).mockResolvedValue(response);
    const phase1Request = { ...request, payload };

    await GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", phase1Request);

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("mutate_graph_document", {
      projectInstanceId,
      graphPath,
      locale: "en-US",
      request: phase1Request,
    });
  });

  it("forwards the exact InsertReroute wire payload once", async () => {
    const response = graphMutationWireResult();
    vi.mocked(invoke).mockResolvedValue(response);
    const rerouteRequest: MutationRequestDto<EditorGraphMutationDto> = {
      ...request,
      payload: {
        type: "insertReroute",
        payload: {
          connectionId: "edge-1",
          position: { x: 120, y: 80 },
        },
      },
    };

    await GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", rerouteRequest);

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke).toHaveBeenCalledWith("mutate_graph_document", {
      projectInstanceId,
      graphPath,
      locale: "en-US",
      request: rerouteRequest,
    });
  });

  it("rejects malformed InsertReroute wire before invoke", async () => {
    const malformedRequest = {
      ...request,
      payload: {
        type: "insertReroute",
        payload: { connectionId: "edge-1", position: { x: Infinity, y: 80 } },
      },
    } as MutationRequestDto<EditorGraphMutationDto>;

    await expect(
      GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", malformedRequest),
    ).rejects.toThrow("InsertReroute");

    expect(invoke).not.toHaveBeenCalled();
  });

  it("sends the captured project identity and parses the mutation result boundary", async () => {
    const response = graphMutationWireResult();
    vi.mocked(invoke).mockResolvedValue(response);

    await expect(
      GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", request),
    ).resolves.toEqual(response);

    expect(invoke).toHaveBeenCalledWith("mutate_graph_document", {
      projectInstanceId,
      graphPath,
      locale: "en-US",
      request,
    });
  });

  it("rejects a mutation response for a different lifecycle identity", async () => {
    vi.mocked(invoke).mockResolvedValue({
      ...(graphMutationWireResult() as Record<string, unknown>),
      projectInstanceId: "00000000-0000-0000-0000-000000000699",
    });

    await expect(
      GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", request),
    ).rejects.toThrow(/projectInstanceId/);
  });

  it("rejects a mutation response without required lifecycle identity", async () => {
    const { projectInstanceId: _omitted, ...malformed } = graphMutationWireResult() as Record<
      string,
      unknown
    >;
    vi.mocked(invoke).mockResolvedValue(malformed);

    await expect(
      GraphMutationService.mutateGraph(projectInstanceId, graphPath, "en-US", request),
    ).rejects.toThrow(/projectInstanceId/);
  });
});
