// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { chartUi } from "@/features/core/chart/ui";
import { editorUi } from "@/features/core/editor/ui";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useDetailPanelModel } from "./useDetailPanelModel";

vi.mock("@/features/application/editor", () => {
  const resources = { events: {}, functions: {}, dataframes: {} };
  return { useDetailResourceProjection: () => resources };
});
vi.mock("@/features/application/log", () => ({
  useLogStore: (selector: (state: { selectedLog: null }) => unknown) =>
    selector({ selectedLog: null }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("Chart detail subscriptions", () => {
  let host: HTMLDivElement;
  let root: Root;
  let latest: ReturnType<typeof useDetailPanelModel>;

  function DetailModelProbe() {
    latest = useDetailPanelModel();
    return <output>{latest.chartDocument?.chartType ?? latest.model.kind}</output>;
  }

  beforeEach(() => {
    useChartDocumentStore.getState().clear();
    editorUi.clearDetailFocus();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    editorUi.clearDetailFocus();
    useChartDocumentStore.getState().clear();
    host.remove();
  });

  it("opens a chart with a stable document reference and follows subsequent updates", () => {
    const chartPath = "charts/Report.yssbi-chart";
    const document: ChartDocument = {
      schemaVersion: 3,
      revision: 1,
      databaseId: "sales",
      chartType: "scatter",
      encodings: { x: "time", y: "amount" },
    };
    const store = useChartDocumentStore.getState();
    store.upsertDocument(chartPath, document);
    store.setIndex([{ chartPath, name: "Report", ...document }]);
    act(() =>
      root.render(
        <StrictMode>
          <DetailModelProbe />
        </StrictMode>,
      ),
    );

    expect(host.textContent).toBe("empty");
    act(() => editorUi.setDetailFocus({ kind: "chart", chartPath }));
    expect(host.textContent).toBe("scatter");
    expect(latest.chartDocument).toBe(document);
    expect(latest.model).toMatchObject({ kind: "chart", document });

    act(() => store.setIndex([{ chartPath, name: "Renamed report", ...document }]));
    expect(latest.chartName).toBe("Renamed report");
    expect(latest.chartDocument).toBe(document);

    act(() => chartUi.updateDraft(chartPath, { chartType: "line" }));
    expect(host.textContent).toBe("line");
    expect(latest.chartDocument).toBe(useChartDocumentStore.getState().documents[chartPath]);
    expect(document.chartType).toBe("scatter");

    act(() => editorUi.clearDetailFocus());
    expect(host.textContent).toBe("empty");
    expect(latest.chartDocument).toBeNull();
  });
});
