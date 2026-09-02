import { describe, expect, it } from "vitest";
import { enUS } from "@/app/i18n/locales/en-US";
import { zhCN } from "@/app/i18n/locales/zh-CN";
import { normalizeIpcError } from "@/services/ipc";
import {
  GRAPH_DRAFT_ERROR_CODES,
  graphDraftErrorCode,
  graphDraftErrorMessageKey,
  type GraphDraftErrorCode,
} from "./graphDraftError";

const backendDetail = "raw backend detail for 00000000-0000-0000-0000-000000000123";

function backendError(code: string) {
  return normalizeIpcError("transform_graph_draft", {
    code,
    details: { debug: backendDetail },
    incidentId: null,
  });
}

function localizedMessage(locale: typeof enUS | typeof zhCN, code: GraphDraftErrorCode): string {
  return locale.canvas.connection.errors[code];
}

describe("graphDraftErrorMessageKey", () => {
  it.each(GRAPH_DRAFT_ERROR_CODES)("maps %s to safe non-empty copy in both locales", (code) => {
    const error = backendError(code);

    expect(graphDraftErrorCode(error)).toBe(code);
    expect(graphDraftErrorMessageKey(code)).toBe(`canvas.connection.errors.${code}`);
    for (const locale of [enUS, zhCN]) {
      const copy = localizedMessage(locale, code);
      expect(copy.trim()).not.toBe("");
      expect(copy).not.toContain(backendDetail);
      expect(copy).not.toContain("00000000-0000-0000-0000-000000000123");
    }
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
