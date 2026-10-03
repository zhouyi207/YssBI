import { resultSessionFixture } from "@/tests/helpers/resultFixture";
import { describe, expect, it } from "vitest";
import type { ResultDescriptor } from "./types";
import { graphOutputKey, portAddressKey } from "@/features/domain/editorProjection";
import {
  buildPinResultSearchEntry,
  createPinResultSearchProjector,
  filterPinResultSearchEntries,
} from "./pinResultSearch";

function currentResult(portKey: string, resultId: string): ResultDescriptor {
  const graphPath = "events/Main.yssbi-event";
  const port = { kind: "declared" as const, nodeId: `node-${portKey}`, portKey };
  return {
    resultId,
    executionSessionId: resultSessionFixture,
    provenance: {
      runId: resultId,
      createdAtMs: "1000",
      graphPath,
      nodeId: port.nodeId,
      output: { graphPath, port },
    },
    presentation: { kind: "inspector" },
    valueKind: "scalar",
    metadata: null,
    totalCount: 1,
    title: "Result",
  };
}

function searchGraph(results: ResultDescriptor[]) {
  return {
    nodes: Object.fromEntries(
      results.map((result) => [
        result.provenance.nodeId,
        {
          display: {
            title: result.provenance.nodeId,
            userLabel: null,
            iconId: null,
            styleId: null,
          },
          position: { x: 0, y: 0 },
        },
      ]),
    ),
    pins: Object.fromEntries(
      results.map((result) => [
        portAddressKey(result.provenance.output!.port),
        {
          name: "Result",
          display: { label: "Result", instanceLabel: null },
        },
      ]),
    ),
  };
}

describe("pinResultSearch", () => {
  it("builds searchable entries from current result descriptors", () => {
    const result = currentResult("result", "17");
    const output = result.provenance.output!;
    expect(
      buildPinResultSearchEntry(result, { nodeTitle: "OLS Regression", pinName: "Result" }),
    ).toMatchObject({
      id: graphOutputKey(output),
      nodeTitle: "OLS Regression",
      pinName: "Result",
      sourceTitle: "Result",
      ref: { kind: "outputPin", graphPath: output.graphPath, output: output.port },
    });
  });

  it("collects and filters descriptors by semantic labels", () => {
    const results = [currentResult("alpha", "17"), currentResult("beta", "18")];
    const entries = createPinResultSearchProjector()(results, searchGraph(results));
    expect(entries).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, "alpha")).toHaveLength(1);
  });

  it("does not substitute opaque identities for missing labels", () => {
    expect(
      buildPinResultSearchEntry(currentResult("result", "17"), { nodeTitle: "", pinName: "" }),
    ).toMatchObject({ nodeTitle: "", pinName: "" });
  });

  it("shares unchanged search entries while relabeling and removing current outputs", () => {
    const alpha = currentResult("alpha", "17");
    const beta = currentResult("beta", "18");
    const results = [alpha, beta];
    const alphaPinId = portAddressKey(alpha.provenance.output!.port);
    const graph = searchGraph(results);
    const project = createPinResultSearchProjector();
    const first = project(results, graph);
    const moved = {
      ...graph,
      nodes: {
        ...graph.nodes,
        "node-alpha": {
          ...graph.nodes["node-alpha"],
          position: { x: 100, y: 30 },
        },
      },
    };
    expect(project(results, moved)).toBe(first);

    const renamed = {
      ...moved,
      pins: {
        ...graph.pins,
        [alphaPinId]: {
          ...graph.pins[alphaPinId],
          display: { label: "Estimate", instanceLabel: null },
        },
      },
    };
    const next = project(results, renamed);
    expect(next[0]).not.toBe(first[0]);
    expect(next[0].pinName).toBe("Estimate");
    expect(next[1]).toBe(first[1]);
    expect(first[0].pinName).toBe("Result");
    expect(filterPinResultSearchEntries(next, "estimate")).toEqual([next[0]]);

    const removed = project([beta], renamed);
    expect(removed).toEqual([first[1]]);
    expect(removed[0]).toBe(first[1]);
    expect(project([], undefined)).toEqual([]);
  });
});
