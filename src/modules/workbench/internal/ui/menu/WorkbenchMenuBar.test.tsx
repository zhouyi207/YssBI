// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { WorkbenchSemanticMenu } from "./WorkbenchMenuBar";

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

describe("WorkbenchMenuBar", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it("exposes all top-level command groups through one semantic menubar", () => {
    const labels = ["File", "Edit", "Data", "View", "Window", "Tools", "Help"];
    act(() =>
      root.render(
        <WorkbenchSemanticMenu menus={labels.map((label) => ({ id: label, label, items: [] }))} />,
      ),
    );

    const menubar = host.querySelector<HTMLElement>('[role="menubar"]');
    expect(menubar).not.toBeNull();
    expect(
      Array.from(menubar?.querySelectorAll<HTMLElement>('[role="menuitem"]') ?? []).map(
        (item) => item.textContent,
      ),
    ).toEqual(["File", "Edit", "Data", "View", "Window", "Tools", "Help"]);
  });
});
