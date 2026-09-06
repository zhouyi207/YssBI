import { describe, expect, it } from "vitest";
import type { PinResultProjection } from "./executionTypes";
import { pinResultCacheKey } from "./pinResultIndex";
import {
  buildPinResultSearchEntry,
  collectPinResultSearchEntries,
  filterPinResultSearchEntries,
} from "./pinResultSearch";

function currentResult(
  portKey: string,
  resultId: string,
  state: "ready" | "cancelled" = "ready",
): PinResultProjection {
  const output = { kind: "declared" as const, nodeId: `node-${portKey}`, portKey };
  return {
    graphPath: "events/Main.yssbi-event",
    output,
    result: {
      resultId,
      state: { kind: state },
      provenance: {
        runId: `run-${resultId}`,
        activationId: resultId,
        createdAtMs: "1000",
        graphPath: "events/Main.yssbi-event",
        nodeId: output.nodeId,
        output: { graphPath: "events/Main.yssbi-event", port: output },
      },
      presentation: { kind: "inspector" },
      valueKind: "scalar",
      metadata: null,
      totalCount: 1,
      title: "Result",
    },
  };
}

describe("pinResultSearch", () => {
  it("builds searchable exact-result entries from current result projections", () => {
    const projection = currentResult("result", "17");
    const entry = buildPinResultSearchEntry(projection, {
      nodeTitle: "OLS Regression",
      pinName: "Result",
    });

    expect(entry).toMatchObject({
      id: pinResultCacheKey(projection.graphPath, projection.output),
      nodeTitle: "OLS Regression",
      pinName: "Result",
      sourceTitle: "ready · 17",
      ref: { kind: "outputPin", graphPath: projection.graphPath, output: projection.output },
    });
  });

  it("omits outputs without a current result", () => {
    const projection = currentResult("result", "17");
    projection.result = null;
    expect(
      buildPinResultSearchEntry(projection, { nodeTitle: "Node", pinName: "Result" }),
    ).toBeNull();
  });

  it("collects and filters cached current result projections", () => {
    const first = currentResult("alpha", "17");
    const second = currentResult("beta", "18", "cancelled");
    const results = new Map([
      [pinResultCacheKey(first.graphPath, first.output), first],
      [pinResultCacheKey(second.graphPath, second.output), second],
    ]);
    const entries = collectPinResultSearchEntries(results, (projection) => ({
      nodeTitle: projection.output.nodeId,
      pinName:
        projection.output.kind === "declared"
          ? projection.output.portKey
          : projection.output.templateKey,
    }));

    expect(entries).toHaveLength(2);
    expect(filterPinResultSearchEntries(entries, "cancelled")).toHaveLength(1);
    expect(filterPinResultSearchEntries(entries, "alpha")).toHaveLength(1);
  });

  it("does not fall back to opaque identities when semantic labels are unavailable", () => {
    const projection = currentResult("result", "17");

    expect(buildPinResultSearchEntry(projection, { nodeTitle: "", pinName: "" })).toMatchObject({
      nodeTitle: "",
      pinName: "",
    });
  });
});
