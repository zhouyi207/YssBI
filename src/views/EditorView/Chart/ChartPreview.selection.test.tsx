// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import { clearChartPreviewCache } from "@/services/chart/chartPreviewCache";
import { fetchChartPreview } from "@/services/chart/chartPreviewDataService";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain";
import type { ChartModel } from "@/shared/types/visualization";
import { ChartPreview } from "./ChartPreview";

vi.mock("@/services/chart/chartPreviewDataService", () => ({ fetchChartPreview: vi.fn() }));
vi.mock("react-i18next", () => ({
  useTranslation: () => ({
    t: (key: string, options?: { column?: unknown }) =>
      options?.column === undefined
        ? `localized:${key}`
        : `localized:${key}:${String(options.column)}`,
  }),
}));
vi.mock("@/shared/charts/ChartRenderer", () => ({
  ChartRenderer: ({ model, surface }: { model: ChartModel; surface: string }) => (
    <div data-chart-surface={surface}>{`${model.kind} plot`}</div>
  ),
}));
vi.mock("./ChartEmptyState", () => ({ ChartEmptyState: () => <div>empty</div> }));

const projectId = "00000000-0000-0000-0000-000000000701";
const chart: ChartDocument = {
  schemaVersion: 3,
  revision: 0,
  databaseId: "sales",
  chartType: "histogram",
  encodings: { x: "amount" },
};
const chartCases: Array<{
  kind: "histogram" | "line" | "scatter";
  marker: string;
  payload: ChartPreviewPayload;
}> = [
  {
    kind: "histogram",
    marker: "histogram plot",
    payload: {
      kind: "histogram",
      bins: [{ label: "10", count: 2 }],
      xLabel: "amount",
      yLabel: "count",
    },
  },
  {
    kind: "line",
    marker: "line plot",
    payload: {
      kind: "line",
      pair: {
        data: [{ x: 1, y: 2 }],
        xLabel: "time",
        yLabel: "amount",
        xFormat: "number",
        yFormat: "number",
      },
    },
  },
  {
    kind: "scatter",
    marker: "scatter plot",
    payload: {
      kind: "scatter",
      pair: {
        data: [{ x: 1, y: 2 }],
        xLabel: "x",
        yLabel: "y",
        xFormat: "number",
        yFormat: "number",
      },
    },
  },
];
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("ChartPreview", () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.useFakeTimers();
    clearChartPreviewCache();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectId, 0);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.useRealTimers();
  });

  it.each(chartCases)("renders $kind output in the chart region", async ({ marker, payload }) => {
    vi.mocked(fetchChartPreview).mockResolvedValue(payload);

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    const chartRegion = host.querySelector("[data-chart-preview-region]");
    expect(chartRegion).not.toBeNull();
    expect(chartRegion?.textContent).toContain(marker);
    expect(chartRegion?.querySelector('[data-chart-surface="plain"]')).not.toBeNull();
  });

  it("keeps a machine error outside the chart region and includes its incident ID", async () => {
    vi.mocked(fetchChartPreview).mockResolvedValue({
      kind: "error",
      code: "chart_preview_backend_failed",
      incidentId: "incident-preview-42",
    });

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    const errorElement = host.querySelector('[role="alert"]');
    expect(errorElement).not.toBeNull();
    expect(errorElement?.closest("[data-chart-preview-region]")).toBeNull();
    expect(errorElement?.textContent).toContain("localized:chart.previewLoadFailed");
    expect(errorElement?.textContent).toContain("localized:common.errorCode");
    expect(errorElement?.textContent).toContain("chart_preview_backend_failed");
    expect(errorElement?.textContent).toContain("localized:common.incidentId");
    expect(errorElement?.textContent).toContain("incident-preview-42");
  });

  it("localizes safe missing-column context without transport prose", async () => {
    vi.mocked(fetchChartPreview).mockResolvedValue({
      kind: "error",
      code: "chart_preview_column_not_found",
      incidentId: null,
      column: "amount",
    });

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    const text = host.querySelector('[role="alert"]')?.textContent;
    expect(text).toContain("localized:chart.previewColumnNotFound:amount");
    expect(text).toContain("chart_preview_column_not_found");
    expect(text).not.toContain("localized:common.incidentId");
  });

  it("maps a rejected raw transport error without displaying its text", async () => {
    vi.mocked(fetchChartPreview).mockRejectedValue(new Error("private chart transport failure"));

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    const text = host.querySelector('[role="alert"]')?.textContent;
    expect(text).toContain("localized:chart.previewLoadFailed");
    expect(text).toContain("chart_preview_read_failed");
    expect(text).not.toContain("private chart transport failure");
  });

  it("keeps a fetched empty preview outside any chart region", async () => {
    vi.mocked(fetchChartPreview).mockResolvedValue({ kind: "empty" });

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    expect(host.textContent).toContain("empty");
    expect(host.querySelector("[data-chart-preview-region]")).toBeNull();
  });
});
