import { describe, expect, it } from "vitest";
import type { RawExpressionDTO, SymbolDraftDTO } from "@/shared/types/bayes";
import {
  bindResponseExpression,
  collectRawSymbols,
  createSymbolDrafts,
  responseBaseNameFromRaw,
} from "./expressionSymbols";

const existing: SymbolDraftDTO[] = [
  { name: "old", role: "parameter", inferredRole: "parameter", userEdited: true },
  { name: "x", role: "parameter", inferredRole: "independent", userEdited: true },
];

describe("createSymbolDrafts", () => {
  it("keeps only symbols from the current parsed formula", () => {
    expect(createSymbolDrafts(["y", "x"], existing).map((symbol) => symbol.name)).toEqual([
      "x",
      "y",
    ]);
  });

  it("preserves configuration only for symbols still present", () => {
    expect(createSymbolDrafts(["x"], existing)).toEqual([existing[1]]);
  });
});

describe("response expressions", () => {
  const rawResponse: RawExpressionDTO = {
    type: "call",
    function: "ln",
    args: [{ type: "symbol", name: "y" }],
  };

  it("extracts only the base symbol from ln(y)", () => {
    expect(collectRawSymbols(rawResponse)).toEqual(["y"]);
    expect(responseBaseNameFromRaw(rawResponse)).toBe("y");
  });

  it("binds the response symbol as a data variable regardless of prior roles", () => {
    expect(bindResponseExpression(rawResponse)).toEqual({
      type: "call",
      function: "ln",
      args: [{ type: "data_variable", name: "y" }],
    });
  });

  it("does not retain symbols absent from the rebuilt expression set", () => {
    expect(
      createSymbolDrafts(
        [...collectRawSymbols(rawResponse), "x", "sigma"],
        [
          ...existing,
          { name: "y", role: "parameter", inferredRole: "parameter", userEdited: true },
        ],
      ).map((symbol) => symbol.name),
    ).toEqual(["sigma", "x", "y"]);
  });
});
