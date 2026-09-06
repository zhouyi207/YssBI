import { describe, expect, it } from "vitest";
import { bayesOverallProgress, formatDuration } from "./BayesProgressStatus";

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

  it("formats elapsed and remaining durations without losing hours", () => {
    expect(formatDuration(5)).toBe("00:05");
    expect(formatDuration(125)).toBe("02:05");
    expect(formatDuration(3_725)).toBe("1:02:05");
  });
});
