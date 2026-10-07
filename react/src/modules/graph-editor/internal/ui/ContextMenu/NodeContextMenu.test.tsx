// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { NodeContextMenu } from "./NodeContextMenu";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

let container: HTMLDivElement;
let portal: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement("div");
  portal = document.createElement("div");
  portal.id = "portal";
  document.body.append(container, portal);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  portal.remove();
});

describe("NodeContextMenu", () => {
  it.each([
    { name: "ordinary node", managed: false, enabled: true },
    { name: "missing projection", managed: undefined, enabled: false },
    { name: "managed node", managed: true, enabled: false },
  ])("enables Duplicate according to ownership for $name", ({ managed, enabled }) => {
    const onDuplicate = vi.fn();
    renderMenu(managed, { onDuplicate });

    const duplicate = item("duplicate")!;
    expect(duplicate.hasAttribute("data-disabled")).toBe(!enabled);
    act(() => duplicate.click());
    expect(onDuplicate).toHaveBeenCalledTimes(enabled ? 1 : 0);
  });

  it("preserves ownership and link state for the other supported actions", () => {
    renderMenu(false, { hasLinks: false });

    expect(item("copy")?.hasAttribute("data-disabled")).toBe(false);
    expect(item("cut")?.hasAttribute("data-disabled")).toBe(false);
    expect(item("delete")?.hasAttribute("data-disabled")).toBe(false);
    expect(item("breakAllLinks")?.hasAttribute("data-disabled")).toBe(true);
    expect(item("selectLinkedNodes")?.hasAttribute("data-disabled")).toBe(true);
  });
});

function item(label: string): HTMLElement | undefined {
  return [...portal.querySelectorAll<HTMLElement>('[role="menuitem"]')].find((button) =>
    button.textContent?.includes(`contextMenu.node.${label}`),
  );
}

function renderMenu(
  managed: boolean | undefined,
  overrides: Partial<React.ComponentProps<typeof NodeContextMenu>> = {},
): void {
  act(() => {
    root.render(
      <NodeContextMenu
        position={{ x: 0, y: 0 }}
        managed={managed}
        hasLinks={false}
        onCopy={vi.fn()}
        onCut={vi.fn()}
        onDuplicate={vi.fn()}
        onDelete={vi.fn()}
        onBreakAllLinks={vi.fn()}
        onSelectLinked={vi.fn()}
        onClose={vi.fn()}
        {...overrides}
      />,
    );
  });
}
