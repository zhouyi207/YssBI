// @vitest-environment happy-dom
import { resultSessionFixture } from "@/tests/helpers/resultFixture";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { DataSeriesResultView } from "./ResultRenderers";
import type { ResultDescriptor } from "../../types";

const paging = vi.hoisted(() => ({
  pageIndex: 0,
  totalPages: 3,
  totalCount: 401,
  actualCount: 200,
  hasMore: true,
  pageSize: 200,
  loading: false,
  values: [1, 2],
  error: null,
  goToPreviousPage: vi.fn(),
  goToNextPage: vi.fn(),
}));
vi.mock("../../usePagedResultRows", () => ({ usePagedResultRows: () => paging }));
vi.mock("../../runtime", () => ({ resultQueryCoordinator: {}, resultQueryRead: {} }));
vi.mock("../ReadOnlyDataGrid", () => ({ ReadOnlyDataGrid: () => null }));
vi.mock("../JsonTreeView", () => ({ JsonTreeView: () => null }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("makes later DataSeries pages accessible through the existing paging actions", () => {
  const payload: ResultDescriptor = {
    resultId: "1",
    title: "Series",
    presentation: { kind: "inspector" },
    valueKind: "dataSeries",
    metadata: null,
    totalCount: 401,
    executionSessionId: resultSessionFixture,
    provenance: {
      runId: "1",
      graphPath: "events/main.yssbi-event",
      nodeId: "node",
      output: null,
      createdAtMs: "0",
    },
  };
  const host = document.createElement("div");
  const root = createRoot(host);
  try {
    act(() => root.render(<DataSeriesResultView payload={payload} />));
    expect(host.textContent).toContain("1–200 of 401");
    const next = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "Next",
    );
    expect(next?.disabled).toBe(false);
    act(() => next!.click());
    expect(paging.goToNextPage).toHaveBeenCalledOnce();
  } finally {
    act(() => root.unmount());
  }
});
