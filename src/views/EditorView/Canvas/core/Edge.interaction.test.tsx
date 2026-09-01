// @vitest-environment happy-dom

import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Edge } from "./Edge";

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
});

function renderEdge(props: Partial<React.ComponentProps<typeof Edge>> = {}) {
  act(() => {
    root.render(
      <svg>
        <Edge
          edgeId="edge-a"
          x1={10}
          y1={20}
          x2={110}
          y2={80}
          color="#123456"
          onPointerDown={() => {}}
          onContextMenu={() => {}}
          {...props}
        />
      </svg>,
    );
  });
}

describe("Edge interaction rendering", () => {
  it("renders one interactive hit path for the edge", () => {
    renderEdge();

    const group = container.querySelector('[data-edge-id="edge-a"]')!;
    const paths = [...group.querySelectorAll("path")];
    const hit = group.querySelector('[data-edge-hit-target="edge-a"]') as SVGPathElement;
    const visible = paths.find((path) => path !== hit)!;

    expect(hit).not.toBeNull();
    expect(hit.getAttribute("pointer-events")).toBe("stroke");
    expect(hit.getAttribute("d")).toBe(visible.getAttribute("d"));
    expect(paths.filter((path) => path.getAttribute("pointer-events") === "stroke")).toEqual([hit]);
  });

  it("routes pointer, click, context, and double-click events only through the hit path", () => {
    const onPointerDown = vi.fn();
    const onClick = vi.fn();
    const onContextMenu = vi.fn();
    const onDoubleClick = vi.fn();
    renderEdge({ onPointerDown, onClick, onContextMenu, onDoubleClick });

    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;
    act(() => hit.dispatchEvent(new PointerEvent("pointerdown", { bubbles: true, button: 0 })));
    act(() => hit.dispatchEvent(new MouseEvent("click", { bubbles: true, button: 0, detail: 1 })));
    act(() => hit.dispatchEvent(new MouseEvent("contextmenu", { bubbles: true, button: 2 })));
    act(() => hit.dispatchEvent(new MouseEvent("dblclick", { bubbles: true, button: 0 })));

    expect(onPointerDown).toHaveBeenCalledTimes(1);
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(onContextMenu).toHaveBeenCalledTimes(1);
    expect(onDoubleClick).toHaveBeenCalledTimes(1);
    expect(container.querySelectorAll("[data-edge-hit-target]")).toHaveLength(1);
  });
});
