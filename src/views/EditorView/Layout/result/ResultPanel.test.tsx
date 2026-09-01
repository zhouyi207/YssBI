// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import type { IDockviewPanelProps } from "dockview-react";
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from "vitest";

import type {
  ResultPanelMetadata,
  WorkbenchPanelMetadata,
  WorkbenchPanelParams,
} from "@/features/core/dockview";

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

function resultMetadata(resultId: string): ResultPanelMetadata {
  return {
    role: "result",
    resultKey: "output:shared",
    resultId,
    title: `Result ${resultId}`,
    presentation: { kind: "inspector" },
    source: null,
  };
}

function panelProps(metadata: WorkbenchPanelMetadata): IDockviewPanelProps<WorkbenchPanelParams> {
  return { params: { metadata } } as unknown as IDockviewPanelProps<WorkbenchPanelParams>;
}

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

  it("ignores non-results and remounts the same result key for a new result ID", () => {
    act(() => root.render(<ResultPanel {...panelProps({ role: "view", viewId: "logs" })} />));
    expect(container.childElementCount).toBe(0);
    expect(mocks.resultContent).not.toHaveBeenCalled();

    act(() => root.render(<ResultPanel {...panelProps(resultMetadata("result-a"))} />));
    expect(container.querySelector("[data-workbench-result-panel]")).not.toBeNull();
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-a",
        mountedResultId: "result-a",
      },
    });

    act(() => root.render(<ResultPanel {...panelProps(resultMetadata("result-b"))} />));
    expect(container.querySelector('[data-testid="result-content"]')).toMatchObject({
      dataset: {
        resultId: "result-b",
        mountedResultId: "result-b",
      },
    });
    expect(mocks.resultContent).toHaveBeenLastCalledWith({ resultId: "result-b" });
  });
});
