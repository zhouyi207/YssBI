// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { useExecutionStore } from "@/features/core/execution";
import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { portAddressKey } from "@/features/domain/editorProjection";
import { OutputPanel } from "./OutputPanel";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";
const sourceGraphPath = "functions/Nested.yssbi-function";
const sourceNodeId = "00000000-0000-0000-0000-000000000002";
const sourcePort = { kind: "declared" as const, nodeId: sourceNodeId, portKey: "message" };

const sourceBucket = {
  nodes: {
    [sourceNodeId]: {
      id: sourceNodeId,
      title: "Print",
      display: { title: "Print" },
    },
  },
  pins: {
    [portAddressKey(sourcePort)]: {
      id: portAddressKey(sourcePort),
      nodeId: sourceNodeId,
      name: "raw-message",
      display: { label: "Message", instanceLabel: null },
      address: sourcePort,
    },
  },
} as unknown as GraphEntityBucket;

describe("OutputPanel", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useExecutionStore.setState({ graphs: {}, playbackGraphPath: null, isPlaying: false });
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphSessionStore.getState().reset();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("renders ordered program output with semantic source labels", () => {
    const execution = useExecutionStore.getState();
    act(() => {
      useGraphDataStore.setState({ graphEntities: { [sourceGraphPath]: sourceBucket } });
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
          <OutputPanel />
        </TooltipProvider>,
      );
    });

    expect(host.textContent).toContain("hello from nested graph");
    expect(host.textContent).toContain("Print · Message");
    expect(host.textContent).not.toContain(sourceGraphPath);
    expect(host.textContent).not.toContain(sourceNodeId);
    expect(host.textContent).toContain("panel.outputTruncated");
  });

  it("renders a shared header without exposing the focused graph path", () => {
    act(() => {
      useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);
      root.render(
        <TooltipProvider>
          <OutputPanel />
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
          <OutputPanel />
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
