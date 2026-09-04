// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { buildGraphResourceMeta, useResourceStore } from "@/features/core/resource";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";
import { useGraphProjectionStore } from "./graphProjectionStore";
import { useNodeView } from "./useNodeView";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

const graphPath = "events/Main.yssbi-event";
const nodeId = "call-1";
const functionPath = "functions/Target.yssbi-function";

function NodeTitleProbe() {
  const node = useNodeView(nodeId, graphPath);
  return <span data-testid="node-title">{node?.title ?? "(no node)"}</span>;
}

function installCallProjection(nodeTypeId: string): void {
  const fixture = makeEditorProjectionFixture({
    graphPath,
    nodeId,
    nodeTypeId,
    title: "Calculate Sales",
  });
  useGraphProjectionStore.getState().replaceProjection(graphPath, fixture.projection);
}

describe("useNodeView Call Function title projection", () => {
  let container: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    useGraphProjectionStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
    container = document.createElement("div");
    document.body.appendChild(container);
    root = createRoot(container);
  });

  afterEach(() => {
    act(() => root.unmount());
    container.remove();
    useGraphProjectionStore.setState({ graphEntities: {} });
    useResourceStore.getState().clear();
  });

  it("keeps the projected call title when the resource store is mismatched", () => {
    useResourceStore.getState().setSnapshot({
      resources: [buildGraphResourceMeta("function", functionPath, "Stale resource title")],
    });
    installCallProjection("yssbi.project.function.call");

    act(() => root.render(<NodeTitleProbe />));

    expect(container.querySelector('[data-testid="node-title"]')?.textContent).toBe(
      "Calculate Sales",
    );
  });

  it("keeps the projected call title when the resource store is empty", () => {
    installCallProjection("yssbi.project.function.call");

    act(() => root.render(<NodeTitleProbe />));

    expect(container.querySelector('[data-testid="node-title"]')?.textContent).toBe(
      "Calculate Sales",
    );
  });
});
