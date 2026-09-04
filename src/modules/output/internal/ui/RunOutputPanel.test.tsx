// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { useExecutionStore } from "@/features/core/execution";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { RunOutputPanel } from "./RunOutputPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";
const sourceGraphPath = "functions/Nested.yssbi-function";
const sourceNodeId = "00000000-0000-0000-0000-000000000002";
const sourcePort = { kind: "declared" as const, nodeId: sourceNodeId, portKey: "message" };

describe("RunOutputPanel", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useExecutionStore.setState({ graphs: {}, playbackGraphPath: null, isPlaying: false });
    useGraphSessionStore.getState().reset();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders ordered program output from the execution event identity", () => {
    const execution = useExecutionStore.getState();
    act(() => {
      useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);
      execution.startExecution(graphPath);
      execution.setActiveRunId(graphPath, "41");
      execution.recordRunOutput(graphPath, {
        runId: "41",
        sequence: 1,
        stream: "stdout",
        text: "hello from nested graph",
        sourceGraphPath,
        sourceNodeId,
        sourcePort,
      });
      execution.recordRunOutput(graphPath, {
        runId: "41",
        sequence: 2,
        stream: "stdout",
        status: "truncated",
        sourceGraphPath,
        sourceNodeId,
        sourcePort,
      });
      execution.completeExecution(graphPath);
      root.render(
        <TooltipProvider>
          <RunOutputPanel />
        </TooltipProvider>,
      );
    });

    expect(host.textContent).toContain("hello from nested graph");
    expect(host.textContent).toContain(`${sourceGraphPath} · ${sourceNodeId} · message`);
    expect(host.textContent).toContain("panel.outputTruncated");
  });

  it("renders a shared header without exposing the focused graph path", () => {
    act(() => {
      useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);
      root.render(
        <TooltipProvider>
          <RunOutputPanel />
        </TooltipProvider>,
      );
    });

    const header = host.querySelector("[data-output-panel-header]");
    expect(header?.textContent).toContain("panel.output");
    expect(header?.textContent).not.toContain(graphPath);
  });

  it("clears only the visible output projection", () => {
    const execution = useExecutionStore.getState();
    act(() => {
      useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);
      execution.startExecution(graphPath);
      execution.setActiveRunId(graphPath, "41");
      execution.recordRunOutput(graphPath, {
        runId: "41",
        sequence: 1,
        stream: "stderr",
        text: "visible output",
        sourceGraphPath: graphPath,
        sourceNodeId,
        sourcePort,
      });
      execution.completeExecution(graphPath);
      root.render(
        <TooltipProvider>
          <RunOutputPanel />
        </TooltipProvider>,
      );
    });

    const clear = host.querySelector<HTMLButtonElement>('button[aria-label="panel.outputClear"]');
    expect(clear).not.toBeNull();
    act(() => clear?.click());

    expect(host.textContent).not.toContain("visible output");
    expect(host.textContent).toContain("panel.outputEmpty");
  });
});
