// @vitest-environment happy-dom

import React, { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
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
          interactive
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
});
