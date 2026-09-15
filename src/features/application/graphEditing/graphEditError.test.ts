import { describe, expect, it } from "vitest";
import { normalizeIpcError } from "@/services/ipc";
import {
  GRAPH_EDIT_ERROR_CODES,
  graphEditErrorCode,
  graphEditErrorMessageKey,
} from "./graphEditError";

const backendDetail = "raw backend detail for 00000000-0000-0000-0000-000000000123";

function backendError(code: string) {
  return normalizeIpcError("edit_graph", {
    code,
    details: { debug: backendDetail },
    incidentId: null,
  });
}

describe("graphEditErrorMessageKey", () => {
  it.each(GRAPH_EDIT_ERROR_CODES)("recognizes the stable rejection code %s", (code) => {
    const error = backendError(code);

    expect(graphEditErrorCode(error)).toBe(code);
  });

  it("returns null for an unknown code value", () => {
    expect(graphEditErrorMessageKey("internal_error")).toBeNull();
  });

  it.each([
    null,
    new Error(backendDetail),
    backendError("internal_error"),
    { code: "graph_connection_type_mismatch", details: null, incidentId: null },
  ])("returns null for an unrecognized rejection %#", (error) => {
    expect(graphEditErrorCode(error)).toBeNull();
  });
});
