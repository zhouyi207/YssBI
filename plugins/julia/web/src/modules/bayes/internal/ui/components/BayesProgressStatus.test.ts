import { describe, expect, it } from "vitest";
import { bayesOverallProgress } from "./BayesProgressStatus";

describe("Bayes progress presentation", () => {
  it("reserves progress milestones for output parsing and frontend rendering", () => {
    expect(bayesOverallProgress("initializing_nuts")).toBe(8);
    expect(bayesOverallProgress("sampling", 300, 300)).toBe(90);
    expect(bayesOverallProgress("summarizing")).toBe(92);
    expect(bayesOverallProgress("posterior_predictive")).toBe(96);
    expect(bayesOverallProgress("reading_result")).toBe(98);
    expect(bayesOverallProgress("writing_artifacts")).toBe(98);
    expect(bayesOverallProgress("rendering_result")).toBe(99);
  });
});
