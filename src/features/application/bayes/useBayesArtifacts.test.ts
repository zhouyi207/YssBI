// @vitest-environment happy-dom
import { act, createElement } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { InferenceResultDTO, TracePlotDataDTO } from "@/shared/types/bayes";
import { useDatabaseStore } from "@/features/core/dataStore/databaseStore";
import {
  clearProjectLifecycle,
  startProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { useBayesArtifacts, type BayesArtifactsModel } from "./useBayesArtifacts";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const mocks = vi.hoisted(() => ({
  savePathDialog: vi.fn(),
  exportBayesArtifactCsv: vi.fn(),
  readBayesTracePlotData: vi.fn(),
  readBayesDensityPlotData: vi.fn(),
  readBayesAutocorrelationData: vi.fn(),
  readBayesPosteriorPredictive: vi.fn(),
  revealPath: vi.fn(),
}));

vi.mock("@/services/bayes", () => ({
  exportBayesArtifactCsv: mocks.exportBayesArtifactCsv,
  readBayesTracePlotData: mocks.readBayesTracePlotData,
  readBayesDensityPlotData: mocks.readBayesDensityPlotData,
  readBayesAutocorrelationData: mocks.readBayesAutocorrelationData,
  readBayesPosteriorPredictive: mocks.readBayesPosteriorPredictive,
}));

vi.mock("@/services/platform/pathDialog", () => ({ savePathDialog: mocks.savePathDialog }));
vi.mock("@/services/platform/opener", () => ({ revealPath: mocks.revealPath }));

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

const result: InferenceResultDTO = {
  summaries: [
    {
      parameter: "alpha",
      mean: 1,
      sd: 0.1,
      median: 1,
      q025: 0.8,
      q975: 1.2,
      rhat: null,
      essBulk: null,
      essTail: null,
    },
  ],
  diagnostics: {
    chains: 1,
    drawsPerChain: 1,
    warmup: 0,
    divergences: null,
    maxTreedepthHits: null,
    warnings: [],
  },
  artifactManifest: {
    taskId: "task-42",
    artifacts: [
      {
        kind: "posterior_samples",
        format: "arrow_ipc",
        path: "results/samples.arrow",
        rows: 1,
      },
    ],
  },
};

const trace: TracePlotDataDTO = {
  series: [],
  maxPointsPerChain: 1,
  stride: 1,
};

describe("useBayesArtifacts", () => {
  let host: HTMLDivElement;
  let root: Root;
  let model!: BayesArtifactsModel;
  const publications: Array<{ loading: boolean; issue: string | null }> = [];

  function Harness() {
    model = useBayesArtifacts({
      result,
      exportKind: "posterior_samples",
      exportFileName: "posterior.csv",
      exportDialogTitle: "Export posterior samples",
    });
    publications.push({ loading: model.loading, issue: model.issue?.code ?? null });
    return null;
  }

  beforeEach(() => {
    vi.clearAllMocks();
    useDatabaseStore.setState({ databases: {}, revisions: {} });
    startProjectLifecycle("project-1");
    mocks.exportBayesArtifactCsv.mockResolvedValue(undefined);
    mocks.revealPath.mockResolvedValue({ ok: true, value: undefined });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    publications.length = 0;
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
    clearProjectLifecycle();
  });

  it("treats a cancelled save dialog as cancelled without exporting", async () => {
    mocks.savePathDialog.mockResolvedValue({ ok: true, value: null });

    await act(async () => root.render(createElement(Harness)));
    let outcome: Awaited<ReturnType<BayesArtifactsModel["exportCsv"]>> | undefined;
    await act(async () => {
      outcome = await model.exportCsv();
    });

    expect(outcome).toEqual({ status: "cancelled" });
    expect(mocks.exportBayesArtifactCsv).not.toHaveBeenCalled();
    expect(model.issue).toBeNull();
  });

  it("does not publish a stale artifact success or failure after project and database generations change", async () => {
    const request = deferred<TracePlotDataDTO>();
    mocks.readBayesTracePlotData.mockReturnValue(request.promise);

    await act(async () => root.render(createElement(Harness)));
    const completion = model.readTrace();
    await act(async () => Promise.resolve());
    const publicationCountBeforeStaleResolution = publications.length;

    startProjectLifecycle("project-2");
    useDatabaseStore.setState({ revisions: { sales: 2 } });
    request.resolve(trace);

    await act(async () => {
      await expect(completion).resolves.toEqual({ status: "stale" });
    });

    expect(
      publications
        .slice(publicationCountBeforeStaleResolution)
        .every((publication) => publication.issue === null && publication.loading),
    ).toBe(true);
    expect(model.issue).toBeNull();
  });
});
