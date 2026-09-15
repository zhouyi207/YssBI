import { describe, expect, it } from "vitest";
import { parseDidPlaceboFakeGroupBlock } from "./did";

const success = {
  available: true,
  observed_coef: 1.75,
  n_perm: 20,
  n_perm_valid: 18,
  min_valid_permutations: 10,
  n_entities: 8,
  n_treated_entities: 3,
  p_value_ri: 0.21,
  perm_coef_mean: 0.04,
  perm_coef_std: 0.32,
};

const unavailable = {
  available: false,
  unavailableCode: "insufficient_valid_permutations",
  n_perm: 9,
  n_perm_valid: 9,
  min_valid_permutations: 10,
  n_entities: 8,
  n_treated_entities: 3,
};

describe("parseDidPlaceboFakeGroupBlock", () => {
  it("accepts the exact structured success shape", () => {
    expect(parseDidPlaceboFakeGroupBlock(success)).toEqual(success);
  });

  it.each([
    ["no_treated_entities", 0, 0],
    ["all_entities_treated", 8, 0],
    ["insufficient_valid_permutations", 3, 9],
  ] as const)("accepts the %s unavailable shape", (unavailableCode, nTreated, nValid) => {
    const value = {
      ...unavailable,
      unavailableCode,
      n_perm_valid: nValid,
      n_treated_entities: nTreated,
    };

    expect(parseDidPlaceboFakeGroupBlock(value)).toEqual(value);
  });

  it("rejects unknown codes, unknown fields, and inconsistent variants", () => {
    expect(
      parseDidPlaceboFakeGroupBlock({
        ...unavailable,
        unavailableCode: "engine_failed",
      }),
    ).toBeNull();
    for (const value of [success, unavailable]) {
      expect(parseDidPlaceboFakeGroupBlock({ ...value, extra: true })).toBeNull();
    }
    expect(parseDidPlaceboFakeGroupBlock({ ...success, available: false })).toBeNull();
  });
});
