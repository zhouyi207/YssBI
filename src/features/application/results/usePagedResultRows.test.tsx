// @vitest-environment happy-dom
import { resultReferenceFixture } from "@/tests/helpers/resultFixture";
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import type { ResultPage } from "@/shared/types/domain/result";
import { usePagedResultRows, type PagedResultRowsState } from "./usePagedResultRows";
import type { ResultQueryCoordinator, ResultQueryReadCapability } from "./resultQueryCoordinator";

vi.mock("./runtime", () => ({ resultQueryCoordinator: {}, resultQueryRead: {} }));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

it("loads unknown-length relations and stops at the backend's final page", async () => {
  const pages = new Map<number, ResultPage>();
  const listeners = new Set<() => void>();
  const read: ResultQueryReadCapability = {
    getAnalysis: () => null,
    subscribe: (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    getPage: ({ offset }) => pages.get(offset) ?? null,
    getDescriptor: () => null,
    getValue: () => null,
    getPinResult: () => null,
    getFailure: () => null,
  };
  const loadPage = vi.fn(async ({ resultId, offset, limit }) => {
    const values = offset === 0 ? [[1], [2]] : [[3]];
    pages.set(offset, {
      resultId,
      offset,
      requestedLimit: limit,
      actualCount: values.length,
      totalCount: offset === 0 ? null : 3,
      hasMore: offset === 0,
      nextOffset: offset === 0 ? 2 : null,
      valueKind: "sequence",
      metadata: { columns: [{ name: "value", type: "Int64" }] },
      values,
    });
    listeners.forEach((listener) => listener());
    return { status: "published" as const };
  });
  const coordinator: ResultQueryCoordinator = {
    resetPinResult: () => {},
    isPayloadRetained: () => false,
    loadAnalysis: async () => ({ status: "notReady" }),
    loadPage,
    retainPayload: () => () => {},
    resetProject: () => {},
    resetResult: () => {},
    loadDescriptor: async () => ({ status: "notReady" }),
    loadValue: async () => ({ status: "notReady" }),
    loadPinResult: async () => ({ status: "notReady" }),
  };
  let state!: PagedResultRowsState;
  function Probe() {
    state = usePagedResultRows(resultReferenceFixture("1"), null, 2, { coordinator, read });
    return null;
  }
  const root = createRoot(document.createElement("div"));
  try {
    await act(async () => root.render(<Probe />));
    expect(loadPage).toHaveBeenCalledWith({ ...resultReferenceFixture("1"), offset: 0, limit: 2 });
    expect(state.totalCount).toBeNull();
    expect(state.rows).toEqual([[1], [2]]);
    expect(state.columns).toEqual([{ name: "value", type: "Int64" }]);
    expect(state.hasMore).toBe(true);
    await act(async () => state.goToNextPage());
    expect(state.rows).toEqual([[3]]);
    expect(state.totalCount).toBe(3);
    expect(state.hasMore).toBe(false);
    expect(state.totalPages).toBe(2);
    await act(async () => state.goToNextPage());
    expect(loadPage).toHaveBeenCalledTimes(2);
    expect(state.pageIndex).toBe(1);
  } finally {
    await act(async () => root.unmount());
  }
});
