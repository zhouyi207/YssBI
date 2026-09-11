import { describe, expect, it } from "vitest";
import { parsePresentationWindowQueryFromParts } from "./parsePresentationWindowQuery";

describe("parsePresentationWindowQueryFromParts", () => {
  it("parses resultId and plotType from hash query", () => {
    const resultId = "9007199254740993";
    const reference = resultReferenceFixture(resultId);
    const leaseId = resultLeaseIdFixture(1);
    const params = new URLSearchParams({ ...reference, leaseId, plotType: "scatter" });

    expect(parsePresentationWindowQueryFromParts(`#/inspect?${params.toString()}`)).toEqual({
      reference,
      leaseId,
      plotType: "scatter",
    });
  });
});
import { resultReferenceFixture, resultLeaseIdFixture } from "@/tests/helpers/resultFixture";
