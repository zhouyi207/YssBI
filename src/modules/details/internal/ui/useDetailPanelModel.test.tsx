// @vitest-environment happy-dom

import { act, StrictMode } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { chartUi } from "@/features/core/chart/ui";
import { editorUi } from "@/features/core/editor/ui";
import type { ChartDocument } from "@/shared/types/domain/chart";
import { useDetailPanelModel } from "./useDetailPanelModel";

vi.mock("@/features/core/editor", () => {
  const resources = { eventGraphs: {}, functionGraphs: {}, dataframes: {} };
  return { useEditorCollections: () => resources };
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
    return <output>{latest.kind === "chart" ? latest.document?.chartType : latest.kind}</output>;
  }

  beforeEach(() => {
    useResourceStore.getState().clear();
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
    useResourceStore.getState().clear();
    host.remove();
  });

  it("opens a chart with a stable document reference and follows subsequent updates", () => {
    const chartPath = "charts/Report.yssbi-chart";
    const document: ChartDocument = {
      schemaVersion: 4,
      databaseId: "sales",
      chartType: "scatter",
      encodings: { x: "time", y: "amount" },
    };
    const store = useChartDocumentStore.getState();
    const chartResource = { kind: "chart" as const, id: chartPath };
    useResourceStore.getState().upsertResource({
      ...chartResource,
      name: "Report",
      uri: resourceKey(chartResource),
      revision: 1,
      exists: true,
      loaded: true,
      hasDirtyDocument: false,
      hasStaleDocument: false,
      hasConflictDocument: false,
    });
    store.upsertDocument(chartPath, document);
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
    expect(latest).toMatchObject({ kind: "chart", path: chartPath, name: "Report", document });
    expect(latest.kind === "chart" && latest.document).toBe(document);

    act(() =>
      useResourceStore
        .getState()
        .patchResource(chartResource, { name: "Renamed report", revision: 2 }),
    );
    expect(latest).toMatchObject({ kind: "chart", name: "Renamed report" });
    expect(latest.kind === "chart" && latest.document).toBe(document);

    act(() => chartUi.updateDraft(chartPath, { chartType: "line" }));
    expect(host.textContent).toBe("line");
    expect(latest.kind === "chart" && latest.document).toBe(
      useChartDocumentStore.getState().documents[chartPath],
    );
    expect(document.chartType).toBe("scatter");

    act(() => editorUi.clearDetailFocus());
    expect(host.textContent).toBe("empty");
    expect(latest).toEqual({ kind: "empty" });
  });
});
