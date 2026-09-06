import { describe, expect, it } from "vitest";
import { normalizeIpcError } from "@/services/ipc";
import {
  GRAPH_DRAFT_ERROR_CODES,
  graphDraftErrorCode,
  graphDraftErrorMessageKey,
} from "./graphDraftError";

const backendDetail = "raw backend detail for 00000000-0000-0000-0000-000000000123";

function backendError(code: string) {
  return normalizeIpcError("transform_graph_draft", {
    code,
    details: { debug: backendDetail },
    incidentId: null,
  });
}

describe("graphDraftErrorMessageKey", () => {
  it.each(GRAPH_DRAFT_ERROR_CODES)("recognizes the stable rejection code %s", (code) => {
    const error = backendError(code);

    expect(graphDraftErrorCode(error)).toBe(code);
  });

  it("returns null for an unknown code value", () => {
    expect(graphDraftErrorMessageKey("internal_error")).toBeNull();
  });

  it.each([
    null,
    new Error(backendDetail),
    backendError("internal_error"),
    { code: "graph_connection_type_mismatch", details: null, incidentId: null },
  ])("returns null for an unrecognized rejection %#", (error) => {
    expect(graphDraftErrorCode(error)).toBeNull();
  });
});
