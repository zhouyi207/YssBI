import { describe, expect, it } from "vitest";
import type { ResultDescriptor } from "./types";
import { graphOutputKey } from "@/features/domain/editorProjection";
import {
  buildPinResultSearchEntry,
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
} from "./pinResultSearch";

function currentResult(portKey: string, resultId: string): ResultDescriptor {
  const graphPath = "events/Main.yssbi-event";
  const port = { kind: "declared" as const, nodeId: `node-${portKey}`, portKey };
  return {
    resultId,
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

  it("omits descriptors without an output address", () => {
    const result = currentResult("result", "17");
    result.provenance.output = null;
    expect(buildPinResultSearchEntry(result, { nodeTitle: "Node", pinName: "Result" })).toBeNull();
  });

  it("collects and filters descriptors by semantic labels", () => {
    const results = [currentResult("alpha", "17"), currentResult("beta", "18")];
    const entries = collectPinResultSearchEntries(results, (result) => ({
      nodeTitle: result.provenance.nodeId,
      pinName: result.title,
    }));
    expect(entries).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, "alpha")).toHaveLength(1);
  });

  it("does not substitute opaque identities for missing labels", () => {
    expect(
      buildPinResultSearchEntry(currentResult("result", "17"), { nodeTitle: "", pinName: "" }),
    ).toMatchObject({ nodeTitle: "", pinName: "" });
  });
});
