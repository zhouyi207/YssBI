// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PresentationWindowState } from "./loadPresentationWindow";

const mocks = vi.hoisted(() => ({
  load: vi.fn(),
  resetResult: vi.fn(),
  resetQueries: vi.fn(),
  invalidate: null as ((ids: readonly string[] | null) => void) | null,
  actions: { setTitle: vi.fn(), show: vi.fn() },
}));
vi.mock("./loadPresentationWindow", () => ({ loadPresentationWindow: mocks.load }));
vi.mock("./parsePresentationWindowQuery", () => ({
  parsePresentationWindowQuery: () => ({ resultId: "17", plotType: "scatter" }),
}));
vi.mock("@/features/application/window/usePersistedWindow", () => ({
  usePersistedWindow: () => {},
}));
vi.mock("@/features/application/window/useCurrentWindowActions", () => ({
  useCurrentWindowActions: () => mocks.actions,
}));
vi.mock("@/services/result/resultInvalidationChannel", () => ({
  subscribeResultInvalidation: (listener: (ids: readonly string[] | null) => void) => {
    mocks.invalidate = listener;
    return () => {
      mocks.invalidate = null;
    };
  },
}));
vi.mock("@/features/application/results/runtime", () => ({
  resetResultQuery: mocks.resetResult,
  resultQueryCoordinator: { resetProject: mocks.resetQueries },
}));
import { usePresentationWindow } from "./usePresentationWindow";

const ready: PresentationWindowState = {
  status: "ready",
  descriptor: {
    resultId: "17",
    state: { kind: "ready" },
    provenance: {
      runId: "1",
      activationId: "17",
      graphPath: "events/Main.yssbi-event",
      nodeId: "00000000-0000-0000-0000-000000000002",
      output: null,
      createdAtMs: "1000",
    },
    presentation: { kind: "plot", chart: "scatter" },
    valueKind: "scalar",
    metadata: null,
    totalCount: 1,
    title: "Plot",
  },
  payload: { mode: "plot", chart: "scatter", data: { points: [1, 2, 3] } },
};
let state: PresentationWindowState;
function WindowProbe() {
  state = usePresentationWindow("plot").state;
  return null;
}
let root: Root;
let container: HTMLDivElement;
beforeEach(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
  vi.clearAllMocks();
  container = document.createElement("div");
  root = createRoot(container);
});
afterEach(() => {
  act(() => root.unmount());
  container.remove();
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = false;
});

describe("detached result lifetime", () => {
  it("drops displayed payload and query caches when its output is invalidated", async () => {
    mocks.load.mockResolvedValueOnce(ready);
    await act(async () => {
      root.render(<WindowProbe />);
    });
    expect(state.status).toBe("ready");
    act(() => mocks.invalidate?.(["18"]));
    expect(state.status).toBe("ready");
    act(() => mocks.invalidate?.(["17"]));
    expect(state).toEqual({ status: "not_found" });
    expect(mocks.resetResult).toHaveBeenCalledWith("17");
    expect(mocks.resetQueries).toHaveBeenCalledOnce();
  });

  it("ignores a load that finishes after the project replaces its results", async () => {
    let settle!: (value: PresentationWindowState) => void;
    mocks.load.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          settle = resolve;
        }),
    );
    await act(async () => {
      root.render(<WindowProbe />);
    });
    act(() => mocks.invalidate?.(null));
    await act(async () => {
      settle(ready);
    });
    expect(state).toEqual({ status: "not_found" });
  });
});
