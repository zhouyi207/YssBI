// @vitest-environment happy-dom

import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { useGraphDiagnosticCounts } from "./useGraphDiagnosticCounts";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Caller.yssbi-event";

describe("useGraphDiagnosticCounts", () => {
  let container: HTMLDivElement;
  let root: ReturnType<typeof createRoot>;

  beforeEach(() => {
    useGraphProjectionStore.setState({ graphEntities: {} });
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useGraphProjectionStore.setState({ graphEntities: {} });
  });

  it("counts only diagnostics supplied by the installed Rust projection", () => {
    const observed: { current: Record<string, number> } = { current: {} };
    function Probe() {
      observed.current = useGraphDiagnosticCounts();
      return null;
    }

    const withDiagnostic = makeEditorProjectionFixture({ graphPath });
    withDiagnostic.projection.diagnostics.push({
      code: "compiler.control.call.target_invalid",
      message: "Call target is invalid",
      severity: "error",
      blocking: true,
      location: { kind: "node", nodeId: withDiagnostic.projection.nodes[0].nodeId },
      related: [],
    });

    act(() => {
      useGraphProjectionStore.getState().replaceProjection(graphPath, withDiagnostic.projection, 1);
      root.render(<Probe />);
    });
    expect(observed.current).toEqual({ [graphPath]: 1 });

    const clean = makeEditorProjectionFixture({ graphPath, sourceRevision: 2 });
    act(() => {
      useGraphProjectionStore.getState().replaceProjection(graphPath, clean.projection, 2);
    });
    expect(observed.current).toEqual({});
  });
});
