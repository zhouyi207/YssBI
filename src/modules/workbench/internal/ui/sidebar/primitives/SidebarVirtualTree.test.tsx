// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SidebarVirtualTree } from "./SidebarVirtualTree";

const scrollToIndex = vi.hoisted(() => vi.fn());

vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: (options: { count: number; getItemKey: (index: number) => string | number }) => ({
    getTotalSize: () => options.count * 28,
    getVirtualItems: () =>
      Array.from({ length: options.count }, (_, index) => ({
        index,
        key: options.getItemKey(index),
        start: index * 28,
        size: 28,
      })),
    measureElement: vi.fn(),
    scrollToIndex,
  }),
}));

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const rows = [
  { key: "first", label: "First", depth: 0 },
  { key: "second", label: "Second", depth: 1 },
  { key: "third", label: "Third", depth: 0 },
];

describe("SidebarVirtualTree", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    scrollToIndex.mockReset();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  function renderTree(
    renderRow: (row: (typeof rows)[number]) => React.ReactNode = (row) => <span>{row.label}</span>,
  ): void {
    act(() =>
      root.render(
        <SidebarVirtualTree
          rows={rows}
          ariaLabel="Project resources"
          emptyMessage="No resources"
          getRowKey={(row) => row.key}
          getRowDepth={(row) => row.depth}
          estimateSize={() => 28}
          renderRow={renderRow}
        />,
      ),
    );
  }

  it("provides one treeitem tab stop and moves focus with tree navigation keys", () => {
    renderTree();
    const items = Array.from(host.querySelectorAll<HTMLElement>('[role="treeitem"]'));

    expect(items.map((item) => item.tabIndex)).toEqual([0, -1, -1]);
    expect(items.map((item) => item.getAttribute("aria-level"))).toEqual(["1", "2", "1"]);

    items[0].focus();
    act(() =>
      items[0].dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowDown",
          bubbles: true,
        }),
      ),
    );
    expect(document.activeElement).toBe(items[1]);
    expect(items.map((item) => item.tabIndex)).toEqual([-1, 0, -1]);

    act(() =>
      items[1].dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowUp",
          bubbles: true,
        }),
      ),
    );
    expect(document.activeElement).toBe(items[0]);

    act(() =>
      items[0].dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "End",
          bubbles: true,
        }),
      ),
    );
    expect(document.activeElement).toBe(items[2]);

    act(() =>
      items[2].dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "Home",
          bubbles: true,
        }),
      ),
    );
    expect(document.activeElement).toBe(items[0]);
    expect(scrollToIndex).toHaveBeenLastCalledWith(0, { align: "auto" });
  });

  it("does not intercept navigation keys from nested interactive controls", () => {
    renderTree((row) => <button type="button">Open {row.label}</button>);
    const firstButton = host.querySelector<HTMLButtonElement>("button");
    expect(firstButton).not.toBeNull();

    firstButton!.focus();
    act(() =>
      firstButton!.dispatchEvent(
        new KeyboardEvent("keydown", {
          key: "ArrowDown",
          bubbles: true,
        }),
      ),
    );

    expect(document.activeElement).toBe(firstButton);
    expect(scrollToIndex).not.toHaveBeenCalled();
  });
});
