import { describe, expect, it } from "vitest";
import { toVarStabilityPoints } from "./toVarStabilityPoints";

describe("toVarStabilityPoints", () => {
  it("keeps report values and derives status at the modulus boundary", () => {
    expect(
      toVarStabilityPoints([
        { re: 0.6, im: 0.8, modulus: 1 },
        { re: 0.5, im: 0.5, modulus: 0.999 },
      ]),
    ).toEqual([
      { re: 0.6, im: 0.8, modulus: 1, status: "unstable" },
      { re: 0.5, im: 0.5, modulus: 0.999, status: "stable" },
    ]);
  });
});
