// @vitest-environment happy-dom
import { resultSessionFixture, resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { readFileSync } from "node:fs";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { ChartThemeProvider } from "@/app/providers/ChartThemeProvider";
import { ResultService } from "@/services/result/resultService";
import { loadPresentationWindow } from "@/features/application/presentation/loadPresentationWindow";
import { resetResultQueryProject } from "@/features/application/results/runtime";
import { ReportView } from "./ReportView";

vi.mock("@/services/result/resultSessionChannel", () => ({
  publishResultSessionEnd: vi.fn(),
}));
vi.mock("@/shared/charts/cartesian/ScatterChart", () => ({
  ScatterChart: () => <div>Residual scatter</div>,
}));
vi.mock("./FormulaBlock", () => ({ default: () => <div>Equation</div> }));
vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("loads an OLS overview and requests report data and tests by reference on demand", async () => {
  const report = JSON.parse(
    readFileSync("src/tests/fixtures/node-system-contracts/ols-summary-report.json", "utf8"),
  );
  report.model_basic_info.num_observation = 53940;
  report.observations.rowCount = 53940;
  vi.spyOn(ResultService, "getDescriptor").mockResolvedValue({
    resultId: "17",
    executionSessionId: resultSessionFixture,
    provenance: {
      runId: "1",
      graphPath: "events/report.yssbi-event",
      nodeId: "node",
      output: null,
      createdAtMs: "1",
    },
    presentation: { kind: "report", report: "olsSummary" },
    valueKind: "scalar",
    totalCount: 1,
    metadata: null,
    title: "OLS Summary",
  });
  vi.spyOn(ResultService, "getValue").mockResolvedValue({ kind: "value", value: report });
  vi.spyOn(ResultService, "getPage").mockResolvedValue({
    resultId: "17",
    offset: 0,
    requestedLimit: 200,
    actualCount: 2,
    totalCount: 2,
    hasMore: false,
    nextOffset: null,
    valueKind: "sequence",
    metadata: {
      columns: [
        "variable",
        "coef",
        "std_err",
        "t_value",
        "p_value",
        "confidence_interval_0.025",
        "confidence_interval_0.975",
        "is_significant",
      ].map((name) => ({ name, type: "Float64" })),
    },
    values: [
      ["_cons", 0.5, 0.25, 2, 0.2, 0.01, 0.99, false],
      ["x1", 0.75, 0.5, 1.5, 0.1, -0.23, 1.73, false],
    ],
  });
  vi.spyOn(ResultService, "analyze").mockImplementation(async (_reference, analysis) => {
    if (analysis.kind === "residualPlot")
      return {
        kind: "residualPlot",
        value: {
          points: [
            { observation: 1, x: 1, y: -1 },
            { observation: 53940, x: 2, y: 1 },
          ],
          totalCount: 53940,
          matchedCount: 53940,
          sampled: true,
          sampling: "systematic",
        },
      };
    if (analysis.kind === "hypothesis")
      return {
        kind: "hypothesis",
        value: {
          test_type: "t",
          h0_form: "x1 = 0",
          h1_form: "x1 != 0",
          alternative: "two-sided",
          r_beta_minus_r: 0.75,
          stat: 1.5,
          df1: 1,
          df2: 53938,
          p_value: 0.1,
        },
      };
    return { kind: "acfPacf", value: { acf: [1, 0.1], pacf: [0.1], n: 53940 } };
  });
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  try {
    const ready = await loadPresentationWindow(resultReferenceFixture("17"));
    if (ready.status !== "ready" || ready.payload.mode !== "report")
      throw new Error("missing report overview");
    const data = ready.payload.data;
    await act(async () =>
      root.render(
        <ChartThemeProvider>
          <TooltipProvider>
            <ReportView descriptor={ready.descriptor} report="olsSummary" data={data} />
          </TooltipProvider>
        </ChartThemeProvider>,
      ),
    );
    expect(host.textContent).toContain("53940");
    expect(ResultService.getValue).toHaveBeenCalledTimes(1);
    expect(ResultService.getPage).toHaveBeenCalledExactlyOnceWith(
      resultReferenceFixture("17"),
      0,
      200,
      "coefficients",
    );
    expect(ResultService.analyze).not.toHaveBeenCalled();

    const plot = host.querySelector("details")!;
    await act(async () => {
      plot.open = true;
      plot.dispatchEvent(new Event("toggle"));
    });
    expect(ResultService.analyze).toHaveBeenCalledWith(report.resultRef, {
      kind: "residualPlot",
      maxPoints: 2000,
      xRange: undefined,
    });
    expect(host.textContent).toContain("sampled observations of 53940");

    const generate = [...host.querySelectorAll("button")].find((button) =>
      button.textContent?.includes("生成"),
    )!;
    await act(async () => generate.click());
    expect(ResultService.analyze).toHaveBeenCalledWith(report.resultRef, {
      kind: "acfPacf",
      maxLag: 20,
    });

    const hypothesis = host.querySelector<HTMLInputElement>('input[placeholder^="e.g."]')!;
    await act(async () => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(
        hypothesis,
        "x1 = 0",
      );
      hypothesis.dispatchEvent(new Event("input", { bubbles: true }));
    });
    const run = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Run",
    )!;
    await act(async () => run.click());
    expect(ResultService.analyze).toHaveBeenCalledWith(report.resultRef, {
      kind: "hypothesis",
      hypothesis: "x1 = 0",
    });
    expect(ResultService.getPage).toHaveBeenCalledTimes(1);
  } finally {
    await act(async () => root.unmount());
    resetResultQueryProject();
    host.remove();
    vi.restoreAllMocks();
  }
});
