import { describe, expect, it } from "vitest";

import editorProjection from "@/tests/fixtures/node-system-contracts/editor-projection.json";
import {
  parseGraphProjectionChannelEventDto,
  parseGraphProjectionSnapshotDto,
} from "./graphProjectionChannelWireParser";

const graphPath = "events/contract.yssbi-event";
const publication = {
  projectInstanceId: "project-a",
  graphSessionId: "graph-session-a",
  graphPath,
  requestGeneration: 4,
  replacement: { graphPath, projection: editorProjection },
};

describe("Graph Projection Channel wire parser", () => {
  it("parses complete replacement and snapshot identities", () => {
    expect(
      parseGraphProjectionChannelEventDto({ type: "projectionReplaced", ...publication }),
    ).toEqual({ type: "projectionReplaced", ...publication });
    expect(
      parseGraphProjectionSnapshotDto({
        projectInstanceId: "project-a",
        streamId: "stream-a",
        projections: [publication],
        latestGenerationByGraph: { [graphPath]: 4 },
      }),
    ).toEqual({
      projectInstanceId: "project-a",
      streamId: "stream-a",
      projections: [publication],
      latestGenerationByGraph: { [graphPath]: 4 },
    });
  });

  it("rejects cross-project, cross-graph, and impossible generation snapshots", () => {
    expect(() =>
      parseGraphProjectionSnapshotDto({
        projectInstanceId: "project-b",
        streamId: "stream-a",
        projections: [publication],
        latestGenerationByGraph: { [graphPath]: 4 },
      }),
    ).toThrow("another project");
    expect(() =>
      parseGraphProjectionChannelEventDto({
        type: "projectionReplaced",
        ...publication,
        graphPath: "events/other.yssbi-event",
      }),
    ).toThrow("another graph");
    expect(() =>
      parseGraphProjectionSnapshotDto({
        projectInstanceId: "project-a",
        streamId: "stream-a",
        projections: [publication],
        latestGenerationByGraph: { [graphPath]: 3 },
      }),
    ).toThrow("generation is inconsistent");
  });

  it("preserves an internal failure incident ID on invalidation events", () => {
    const invalidated = {
      type: "projectionInvalidated" as const,
      projectInstanceId: "project-a",
      graphSessionId: "graph-session-a",
      graphPath,
      requestGeneration: 4,
      reasonCode: "graph_projection_resolution_failed",
      incidentId: "incident-42",
    };

    expect(parseGraphProjectionChannelEventDto(invalidated)).toEqual(invalidated);
  });
});
