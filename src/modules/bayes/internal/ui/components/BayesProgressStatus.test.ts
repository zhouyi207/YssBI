import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import {
  bayesOverallProgress,
  bayesProgressStageLabel,
  formatDuration,
} from "./BayesProgressStatus";

const translateKey = ((key: string) => key) as TFunction;

describe("Bayes progress presentation", () => {
  it("translates stable user-facing labels for backend stages", () => {
    expect(bayesProgressStageLabel("loading_model", translateKey)).toBe(
      "bayes.progress.stages.loadingModel",
    );
    expect(bayesProgressStageLabel("loading_data", translateKey)).toBe(
      "bayes.progress.stages.loadingData",
    );
    expect(bayesProgressStageLabel("loading_kernels", translateKey)).toBe(
      "bayes.progress.stages.loadingKernels",
    );
    expect(bayesProgressStageLabel("building_model", translateKey)).toBe(
      "bayes.progress.stages.buildingModel",
    );
    expect(bayesProgressStageLabel("initializing_nuts", translateKey)).toBe(
      "bayes.progress.stages.initializingNuts",
    );
    expect(bayesProgressStageLabel("warmup", translateKey)).toBe("bayes.progress.stages.warmup");
    expect(bayesProgressStageLabel("sampling", translateKey)).toBe(
      "bayes.progress.stages.sampling",
    );
    expect(bayesProgressStageLabel("summarizing", translateKey)).toBe(
      "bayes.progress.stages.summarizing",
    );
    expect(bayesProgressStageLabel("posterior_predictive", translateKey)).toBe(
      "bayes.progress.stages.posteriorPredictive",
    );
    expect(bayesProgressStageLabel("writing_artifacts", translateKey)).toBe(
      "bayes.progress.stages.writingArtifacts",
    );
    expect(bayesProgressStageLabel("rendering_result", translateKey)).toBe(
      "bayes.progress.stages.renderingResult",
    );
  });

  it("falls back to the backend value for unknown stages", () => {
    expect(bayesProgressStageLabel("unknown_stage", translateKey)).toBe("unknown_stage");
  });

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
