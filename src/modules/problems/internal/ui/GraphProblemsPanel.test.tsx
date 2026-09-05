// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useEditorStore } from "@/features/core/editor";
import { useGraphSessionStore } from "@/features/core/graphSession/graphSessionStore";
import { portAddressKey } from "@/features/domain/editorProjection";
import type { DiagnosticDto } from "@/shared/types/dto/editorProjection";
import { GraphProblemsPanel } from "./GraphProblemsPanel";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";

function diagnostic(nodeId: string, code: string, message: string): DiagnosticDto {
  return {
    code: code === "node.error" ? "compiler.node.unknown" : "compiler.input.unbound",
    messageKey:
      code === "node.error"
        ? "diagnostics.compiler.node.unknown"
        : "diagnostics.compiler.input.unbound",
    arguments: { node_type: message, port: message },
    severity: code === "node.error" ? "error" : "warning",
    blocking: code === "node.error",
    location: { kind: "node", nodeId },
    related: [],
  };
}

const canonicalDiagnostics = [
  diagnostic("node-a", "node.error", "A is invalid"),
  diagnostic("node-b", "node.warning", "B needs review"),
];

const bucket = {
  diagnostics: canonicalDiagnostics,
  graphNodes: ["node-a", "node-b"],
  nodes: {
    "node-a": {
      id: "node-a",
      display: { title: "Node A" },
      diagnostics: [],
    },
    "node-b": {
      id: "node-b",
      display: { title: "Node B" },
      diagnostics: [],
    },
  },
} as unknown as GraphEntityBucket;

describe("GraphProblemsPanel", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    clearProjectLifecycle();
    startProjectLifecycle("project-problems");
    useGraphProjectionStore.setState({ graphEntities: {} });
    useEditorStore.getState().clearDetailFocus();
    useGraphSessionStore.getState().reset();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    clearProjectLifecycle();
    act(() => root.unmount());
    host.remove();
  });

  it("lists canonical problems for the focused graph and locates node-owned rows", () => {
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: bucket } });
    useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);

    act(() => {
      root.render(
        <TooltipProvider>
          <GraphProblemsPanel />
        </TooltipProvider>,
      );
    });

    expect(host.querySelectorAll("[data-graph-problem-row]")).toHaveLength(2);
    expect(host.textContent).toContain("Node A");
    expect(host.textContent).toContain("A is invalid");
    expect(host.textContent).toContain("Node B");
    expect(host.textContent).toContain("B needs review");

    const firstRow = host.querySelector<HTMLButtonElement>("[data-graph-problem-row]");
    expect(firstRow).not.toBeNull();
    act(() => firstRow?.click());
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: "node",
      id: "node-a",
      graphPath,
    });
  });

  it("renders a shared header without exposing the focused graph path", () => {
    useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);

    act(() => {
      root.render(
        <TooltipProvider>
          <GraphProblemsPanel />
        </TooltipProvider>,
      );
    });

    const header = host.querySelector("[data-graph-problems-panel-header]");
    expect(header?.textContent).toContain("panel.problems");
    expect(header?.textContent).toContain("panel.problemsCount");
    expect(header?.textContent).not.toContain(graphPath);
  });

  it("shows an empty state when the canonical projection has no problems", () => {
    useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);
    useGraphProjectionStore.setState({
      graphEntities: { [graphPath]: { ...bucket, diagnostics: [] } },
    });

    act(() => {
      root.render(
        <TooltipProvider>
          <GraphProblemsPanel />
        </TooltipProvider>,
      );
    });

    expect(host.querySelectorAll("[data-graph-problem-row]")).toHaveLength(0);
    expect(host.textContent).toContain("panel.problemsEmpty");
  });

  it("shows semantic port locations without exposing node or pin identities", () => {
    const address = { kind: "declared" as const, nodeId: "node-a", portKey: "value" };
    const pinId = portAddressKey(address);
    const portProblem: DiagnosticDto = {
      ...diagnostic("node-a", "node.error", "Value is invalid"),
      location: { kind: "port", address },
    };
    const portBucket = {
      ...bucket,
      graphNodes: ["node-a"],
      diagnostics: [portProblem],
      pins: {
        [pinId]: {
          id: pinId,
          nodeId: "node-a",
          name: "raw-value",
          display: { label: "Value", instanceLabel: null },
          address,
        },
      },
    } as unknown as GraphEntityBucket;
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: portBucket } });
    useGraphSessionStore.getState().setFocusedSession("group-1", graphPath);

    act(() => {
      root.render(
        <TooltipProvider>
          <GraphProblemsPanel />
        </TooltipProvider>,
      );
    });

    expect(host.textContent).toContain("Node A · Value");
    expect(host.textContent).not.toContain("node-a");
    expect(host.textContent).not.toContain(pinId);
  });
});
