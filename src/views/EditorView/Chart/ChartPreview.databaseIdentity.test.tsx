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
vi.mock("@/shared/charts/ChartRenderer", () => ({
  ChartRenderer: ({ model }: { model: ChartModel }) => (
    <div>{model.kind === "scatter" ? `scatter:${model.points[0]?.x}` : model.kind}</div>
  ),
}));
vi.mock("./ChartEmptyState", () => ({ ChartEmptyState: () => <div>empty</div> }));

const projectA = "00000000-0000-0000-0000-000000000601";
const projectB = "00000000-0000-0000-0000-000000000602";
const chart: ChartDocument = {
  schemaVersion: 3,
  revision: 0,
  databaseId: "sales",
  chartType: "scatter",
  encodings: { x: "x", y: "y" },
};
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => {
    resolve = settle;
  });
  return { promise, resolve };
}

function scatter(x: number): ChartPreviewPayload {
  return {
    kind: "scatter",
    pair: {
      data: [{ x, y: x }],
      xLabel: "x",
      yLabel: "y",
      xFormat: "number",
      yFormat: "number",
    },
  };
}

describe("ChartPreview project cache identity", () => {
  let root: Root;
  let host: HTMLDivElement;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.useFakeTimers();
    clearChartPreviewCache();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectA, 0);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.useRealTimers();
  });

  it("does not reuse an old in-flight payload after same-ID project replacement", async () => {
    const oldRequest = deferred<ChartPreviewPayload>();
    vi.mocked(fetchChartPreview)
      .mockReturnValueOnce(oldRequest.promise)
      .mockResolvedValueOnce(scatter(2));

    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));
    expect(fetchChartPreview).toHaveBeenCalledTimes(1);

    act(() => root.unmount());
    host.remove();
    projectPublicationCoordinator.startProject(projectB, 0);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    act(() => root.render(<ChartPreview chartPath="charts/Chart.yssbi-chart" document={chart} />));
    await act(async () => vi.advanceTimersByTimeAsync(300));

    expect(fetchChartPreview).toHaveBeenCalledTimes(2);
    expect(host.textContent).toContain("scatter:2");

    await act(async () => {
      oldRequest.resolve(scatter(1));
      await Promise.resolve();
    });
    expect(host.textContent).toContain("scatter:2");
    expect(host.textContent).not.toContain("scatter:1");
  });
});
