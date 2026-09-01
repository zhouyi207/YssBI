// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  resultContent: vi.fn(),
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

  it("remounts its content for a new result ID", () => {
    act(() => root.render(<ResultPanel resultId="result-a" />));
    expect(container.querySelector("[data-workbench-result-panel]")).not.toBeNull();
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-a",
        mountedResultId: "result-a",
      },
    });

    act(() => root.render(<ResultPanel resultId="result-b" />));
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-b",
        mountedResultId: "result-b",
      },
    });
    expect(mocks.resultContent).toHaveBeenLastCalledWith({ resultId: "result-b" });
  });
});
