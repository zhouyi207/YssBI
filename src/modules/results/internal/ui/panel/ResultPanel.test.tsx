// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  resultContent: vi.fn(),
  currentId: "result-a" as string | null,
  listeners: new Set<() => void>(),
}));

vi.mock("@/features/application/results", () => ({
  readPinResultStatus: () => "running",
  resultQueryRead: {
    subscribe: (listener: () => void) => {
      mocks.listeners.add(listener);
      return () => mocks.listeners.delete(listener);
    },
    getPinResult: () => (mocks.currentId ? { resultId: mocks.currentId } : null),
  },
}));

vi.mock("./ResultContent", async () => {
  const { useState } = await import("react");
  return {
    ResultContent: (props: { resultId: string }) => {
      mocks.resultContent(props);
      const [mountedResultId] = useState(props.resultId);
      return (
        <div
          data-testid="result-content"
          data-result-id={props.resultId}
          data-mounted-result-id={mountedResultId}
        />
      );
    },
  };
});

import { ResultPanel } from "./ResultPanel";

describe("ResultPanel", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (
      globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (
      globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }
    ).IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    vi.clearAllMocks();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
  });

  it("unmounts old content during rerun and follows the current output without reopening the panel", () => {
    const source = {
      graphPath: "events/Main.yssbi-event",
      port: { kind: "declared" as const, nodeId: "node-1", portKey: "result" },
    };
    mocks.currentId = "result-a";
    act(() => root.render(<ResultPanel resultId="result-a" source={source} />));
    expect(container.querySelector("[data-workbench-result-panel]")).not.toBeNull();
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-a",
        mountedResultId: "result-a",
      },
    });

    act(() => {
      mocks.currentId = null;
      mocks.listeners.forEach((listener) => listener());
    });
    expect(container.querySelector('[data-testid="result-content"]')).toBeNull();
    act(() => {
      mocks.currentId = "result-b";
      mocks.listeners.forEach((listener) => listener());
    });
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-b",
        mountedResultId: "result-b",
      },
    });
    expect(mocks.resultContent).toHaveBeenLastCalledWith({ resultId: "result-b" });
  });
});
