// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NodeContextMenu } from "./ContextMenu/NodeContextMenu";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe("unavailable node actions", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.innerHTML = "";
  });

  it.each([
    {
      name: "managed node",
      managed: true,
    },
    {
      name: "node without a projection",
      managed: undefined,
    },
  ])("disables delete and cut for a $name", ({ managed }) => {
    const onCut = vi.fn();
    const onDelete = vi.fn();
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          managed={managed}
          onCopy={vi.fn()}
          onCut={onCut}
          onDuplicate={vi.fn()}
          onDelete={onDelete}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const items = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const cut = items.find((item) => item.textContent?.includes("contextMenu.node.cut"));
    const deleteButton = items.find((item) =>
      item.textContent?.includes("contextMenu.node.delete"),
    );
    expect(cut?.hasAttribute("data-disabled")).toBe(true);
    expect(deleteButton?.hasAttribute("data-disabled")).toBe(true);
    cut?.click();
    deleteButton?.click();
    expect(onCut).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it("enables duplicate for an unmanaged node", () => {
    const onDuplicate = vi.fn();
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          managed={false}
          onCopy={vi.fn()}
          onCut={vi.fn()}
          onDuplicate={onDuplicate}
          onDelete={vi.fn()}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const duplicate = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')].find(
      (button) => button.textContent?.includes("contextMenu.node.duplicate"),
    ) as HTMLButtonElement;
    expect(duplicate.hasAttribute("data-disabled")).toBe(false);
    act(() => duplicate.click());
    expect(onDuplicate).toHaveBeenCalledOnce();
  });

  it("prevents cut and deletion of managed nodes", () => {
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          managed={true}
          onCopy={vi.fn()}
          onCut={vi.fn()}
          onDuplicate={vi.fn()}
          onDelete={vi.fn()}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const items = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')];
    const cut = items.find((item) => item.textContent?.includes("contextMenu.node.cut"));
    const deleteButton = items.find((item) =>
      item.textContent?.includes("contextMenu.node.delete"),
    );
    expect(cut?.hasAttribute("data-disabled")).toBe(true);
    expect(deleteButton?.hasAttribute("data-disabled")).toBe(true);
  });
});
