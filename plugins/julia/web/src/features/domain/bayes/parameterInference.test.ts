import { describe, expect, it } from "vitest";
import { DEFAULT_NORMAL_LIKELIHOOD } from "./likelihoodDefaults";
import { createDefaultParameter } from "./priorDefaults";
import { mergeInferredParameters } from "./parameterInference";

describe("Bayesian parameter inference", () => {
  it("adds inferred parameters and likelihood parameters with defaults", () => {
    const result = mergeInferredParameters([], ["a", "b"], DEFAULT_NORMAL_LIKELIHOOD);

    expect(result.map((parameter) => parameter.name)).toEqual(["a", "b", "sigma"]);
    expect(result.find((parameter) => parameter.name === "a")?.prior).toEqual({
      distribution: "normal",
      args: [0, 10],
    });
    expect(result.find((parameter) => parameter.name === "sigma")?.constraint).toEqual({
      type: "positive",
    });
    expect(result.find((parameter) => parameter.name === "sigma")?.prior).toEqual({
      distribution: "exponential",
      args: [1],
    });
  });

  it("preserves existing parameter settings and removes unused parameters", () => {
    const existing = [
      {
        ...createDefaultParameter("a"),
        prior: { distribution: "normal" as const, args: [0, 5] as [number, number] },
      },
      createDefaultParameter("old"),
    ];

    const result = mergeInferredParameters(existing, ["a"], DEFAULT_NORMAL_LIKELIHOOD);

    expect(result.find((parameter) => parameter.name === "a")?.prior).toEqual({
      distribution: "normal",
      args: [0, 5],
    });
    expect(result.map((parameter) => parameter.name)).toEqual(["a", "sigma"]);
  });
});
