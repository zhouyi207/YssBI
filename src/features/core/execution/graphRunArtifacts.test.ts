import { describe, expect, it } from "vitest";
import { graphHasClearableArtifacts } from "./graphRunArtifacts";
import type { GraphExecutionState } from "./executionTypes";

function graph(partial: Partial<GraphExecutionState>): GraphExecutionState {
  return {
    status: "idle",
    runId: null,
    nodeStates: new Map(),
    completedConnections: new Set(),
    flowingConnections: new Set(),
    recording: [],
    graphDirty: false,
    runOutput: { runId: null, entries: [], projectionDropped: false },
    pinHistories: new Map(),
    pinPreviews: new Map(),
    ...partial,
  };
}

describe("graphHasClearableArtifacts", () => {
  it("returns false when idle with no artifacts", () => {
    expect(graphHasClearableArtifacts(undefined)).toBe(false);
    expect(graphHasClearableArtifacts(graph({}))).toBe(false);
  });

  it("returns false while running", () => {
    expect(
      graphHasClearableArtifacts(
        graph({ status: "running", pinHistories: new Map([["p", {} as never]]) }),
      ),
    ).toBe(false);
  });

  it("returns true when result, recording, or run output projections exist", () => {
    expect(graphHasClearableArtifacts(graph({ pinHistories: new Map([["p", {} as never]]) }))).toBe(
      true,
    );
    expect(
      graphHasClearableArtifacts(
        graph({ recording: [{ event: { event: "executionStart" }, timestamp: 0 }] }),
      ),
    ).toBe(true);
    expect(
      graphHasClearableArtifacts(
        graph({
          runOutput: {
            runId: "41",
            entries: [
              {
                runId: "41",
                sequence: 1,
                stream: "stdout",
                text: "value",
                sourceGraphPath: "events/Main.yssbi-event",
                sourceNodeId: "00000000-0000-0000-0000-000000000002",
                sourcePort: {
                  kind: "declared",
                  nodeId: "00000000-0000-0000-0000-000000000002",
                  portKey: "message",
                },
              },
            ],
            projectionDropped: false,
          },
        }),
      ),
    ).toBe(true);
  });

  it("returns true after completed or error status", () => {
    expect(graphHasClearableArtifacts(graph({ status: "completed" }))).toBe(true);
    expect(graphHasClearableArtifacts(graph({ status: "error" }))).toBe(true);
  });
});
