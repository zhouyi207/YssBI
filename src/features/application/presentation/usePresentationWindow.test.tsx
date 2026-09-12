// @vitest-environment happy-dom
import {
  resultSessionFixture,
  resultReferenceFixture,
  resultLeaseIdFixture,
} from "@/tests/helpers/resultFixture";
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PresentationWindowState } from "./loadPresentationWindow";

const mocks = vi.hoisted(() => ({
  load: vi.fn(),
  resetResult: vi.fn(),
  endSession: null as ((session: string | null) => void) | null,
  claim: vi.fn(),
  actions: { setTitle: vi.fn(), show: vi.fn(), close: vi.fn() },
}));
vi.mock("./loadPresentationWindow", () => ({ loadPresentationWindow: mocks.load }));
vi.mock("./parsePresentationWindowQuery", () => ({
  parsePresentationWindowQuery: () => ({
    reference: resultReferenceFixture("17"),
    leaseId: resultLeaseIdFixture(100),
    plotType: "scatter",
  }),
}));
vi.mock("@/features/application/window/useCurrentWindowActions", () => ({
  useCurrentWindowActions: () => mocks.actions,
}));
vi.mock("@/services/result/resultSessionChannel", () => ({
  subscribeResultSessionEnd: (listener: (session: string | null) => void) => {
    mocks.endSession = listener;
    return () => {
      mocks.endSession = null;
    };
  },
}));
vi.mock("@/features/application/results/runtime", () => ({
  resetResultQuery: mocks.resetResult,
}));
vi.mock("@/services/result/resultService", () => ({ ResultService: { claim: mocks.claim } }));
import { usePresentationWindow } from "./usePresentationWindow";

const ready: PresentationWindowState = {
  status: "ready",
  descriptor: {
    resultId: "17",

    executionSessionId: resultSessionFixture,
    provenance: {
      runId: "1",
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
  state = usePresentationWindow().state;
  return null;
}
let root: Root;
let container: HTMLDivElement;
beforeEach(() => {
  (
    globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
  ).IS_REACT_ACT_ENVIRONMENT = true;
  vi.clearAllMocks();
  if (ready.status === "ready")
    mocks.claim.mockResolvedValue({
      leaseId: resultLeaseIdFixture(100),
      descriptor: ready.descriptor,
    });
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
  it("closes a held window only when its owning execution session ends", async () => {
    mocks.load.mockResolvedValueOnce(ready);
    await act(async () => {
      root.render(<WindowProbe />);
    });
    expect(state.status).toBe("ready");
    act(() => mocks.endSession?.("00000000-0000-0000-0000-000000000002"));
    expect(state.status).toBe("ready");
    act(() => mocks.endSession?.(resultSessionFixture));
    expect(state).toEqual({ status: "not_found" });
    expect(mocks.resetResult).toHaveBeenCalledWith(resultReferenceFixture("17"));
    expect(mocks.actions.close).toHaveBeenCalledOnce();
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
    act(() => mocks.endSession?.(null));
    await act(async () => {
      settle(ready);
    });
    expect(state).toEqual({ status: "not_found" });
  });
});
