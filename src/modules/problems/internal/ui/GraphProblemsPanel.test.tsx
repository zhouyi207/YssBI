// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { GraphEntityBucket } from "@/features/core/dataStore/graphEntityAccess";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useEditorStore } from "@/features/core/editor";
import { workbenchLayoutRead } from "@/modules/workbench/public";
import type { DiagnosticDto } from "@/shared/types/dto/editorProjection";
import { GraphProblemsPanel } from "./GraphProblemsPanel";
import {
  startProjectLifecycle,
  clearProjectLifecycle,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";

function diagnostic(nodeId: string, code: string, message: string): DiagnosticDto {
  return {
    code: code === "node.error" ? "graph.node.unknown" : "graph.input.unbound",
    messageKey:
      code === "node.error" ? "diagnostics.graph.node.unknown" : "diagnostics.graph.input.unbound",
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
    useGraphProjectionStore.getState().clear();
    useEditorStore.getState().clearDetailFocus();
    vi.spyOn(workbenchLayoutRead, "getActiveEditorPanel").mockReturnValue({
      panelInstanceId: "graph-panel",
      groupId: "group-1",
      component: "EditorResource",
      metadata: { role: "editor", resourceKind: "event", resourceRef: graphPath },
      active: true,
      visible: true,
      location: { type: "grid" },
    });
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    clearProjectLifecycle();
    act(() => root.unmount());
    host.remove();
    vi.restoreAllMocks();
  });

  it("lists canonical problems for the native active graph and locates node-owned rows", () => {
    useGraphProjectionStore.setState({ graphEntities: { [graphPath]: bucket } });

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

  it("shows an empty state when the canonical projection has no problems", () => {
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
});
