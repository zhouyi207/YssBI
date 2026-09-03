// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import type { PinView } from "@/features/domain/editorProjection/graphRuntimeTypes";
import type { UINode } from "@/features/core/dataStore/nodeView";
import { makeProjectedPinData } from "@/tests/helpers/editorProjectionFixtures";
import { GraphNodeView } from "./GraphNodeView";
import { RerouteNodeLayout } from "./RerouteNodeLayout";

vi.mock("react-i18next", async (importOriginal) => ({
  ...(await importOriginal<typeof import("react-i18next")>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  document.querySelector("[data-yssbi-overlay-root]")?.remove();
});

function projectedPin(nodeId: string, direction: "input" | "output"): PinView {
  const id = `${nodeId}:${direction}`;
  return {
    ...makeProjectedPinData({
      id,
      nodeId,
      name: direction === "input" ? "Input" : "Output",
      direction,
      dataType: { kind: "Float64" },
      kind: "data",
    }),
    kind: "data",
    address: { kind: "declared", nodeId, portKey: direction },
    resolvedType: { display: "Float64", resolved: true, dataType: { kind: "Float64" } },
    connected: true,
    linkCount: 1,
    connectionIds: [`${nodeId}:connection`],
  };
}

function projectedReroute(): UINode {
  const id = "reroute-data";
  return {
    id,
    nodeType: "yssbi.reroute.data",
    title: "Forbidden reroute title",
    styleId: "builtin.reroute",
    position: { x: 135, y: 246 },
    display: {
      title: "Forbidden reroute title",
      userLabel: null,
      iconId: null,
      styleId: "builtin.reroute",
    },
    parameterEditors: [],
    diagnostics: [],
    inputs: [projectedPin(id, "input")],
    outputs: [projectedPin(id, "output")],
  };
}

function renderNode(node: UINode, onPinPointerDown = vi.fn(), onPointerDown = vi.fn()) {
  act(() =>
    root.render(
      <TooltipProvider>
        <GraphNodeView
          nodeId={node.id}
          className=""
          style={{}}
          contentSlot={<RerouteNodeLayout node={node} onPinPointerDown={onPinPointerDown} />}
          onPointerDown={(event) => onPointerDown(node.id, event)}
          onContextMenu={vi.fn()}
        />
      </TooltipProvider>,
    ),
  );
  return { onPinPointerDown, onPointerDown };
}

describe("RerouteNodeLayout", () => {
  it("renders one connectable data input/output and no ordinary node UI", () => {
    const node = projectedReroute();
    const { onPinPointerDown } = renderNode(node);
    const nodeRoot = container.querySelector(`[data-node-id="${node.id}"]`) as HTMLDivElement;
    expect(container.querySelector("[data-reroute-layout]")).not.toBeNull();
    const pins = [...container.querySelectorAll("[data-pin-id]")] as HTMLDivElement[];

    expect(nodeRoot).not.toBeNull();
    expect(pins.map((pin) => pin.dataset.pinId)).toEqual([`${node.id}:input`, `${node.id}:output`]);
    expect(container.textContent).not.toContain(node.title);
    expect(container.textContent).not.toContain("Hidden");
    expect(container.querySelector("input")).toBeNull();

    const inputAnchor = pins[0].querySelector<HTMLElement>("[data-pin-connection-anchor]");
    expect(inputAnchor).not.toBeNull();
    act(() =>
      inputAnchor!.dispatchEvent(
        new PointerEvent("pointerdown", {
          bubbles: true,
          cancelable: true,
          button: 0,
        }),
      ),
    );
    expect(onPinPointerDown).toHaveBeenCalledOnce();
    expect(onPinPointerDown.mock.calls[0][1]).toMatchObject({
      id: `${node.id}:input`,
      kind: "data",
      address: { kind: "declared", nodeId: node.id, portKey: "input" },
    });
  });
});
