// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { NodeCapabilitiesDto } from "@/shared/types/dto/editorProjection";
import { NodeContextMenu } from "./NodeContextMenu";

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT =
  true;

type Capabilities = Pick<NodeCapabilitiesDto, "managed" | "canCopy" | "canDelete">;

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
  it("omits permanently unsupported actions and retains supported actions", () => {
    renderMenu({ managed: false, canCopy: true, canDelete: true }, { hasLinks: true });

    expect(item("disableNode")).toBeUndefined();
    expect(item("rename")).toBeUndefined();
    expect(item("collapse")).toBeUndefined();

    expect(item("copy")).toBeDefined();
    expect(item("cut")).toBeDefined();
    expect(item("duplicate")).toBeDefined();
    expect(item("selectNode")).toBeUndefined();
    expect(item("breakAllLinks")).toBeDefined();
    expect(item("selectLinkedNodes")).toBeDefined();
    expect(item("delete")).toBeDefined();
  });

  it.each([
    {
      name: "unmanaged copyable node",
      capabilities: { managed: false, canCopy: true, canDelete: false },
      enabled: true,
    },
    {
      name: "unmanaged non-copyable node",
      capabilities: { managed: false, canCopy: false, canDelete: true },
      enabled: false,
    },
    {
      name: "managed copyable node",
      capabilities: { managed: true, canCopy: true, canDelete: true },
      enabled: false,
    },
  ])("enables Duplicate only for an $name", ({ capabilities, enabled }) => {
    const onDuplicate = vi.fn();
    renderMenu(capabilities, { onDuplicate });

    const duplicate = item("duplicate")!;
    expect(duplicate.hasAttribute("data-disabled")).toBe(!enabled);
    act(() => duplicate.click());
    expect(onDuplicate).toHaveBeenCalledTimes(enabled ? 1 : 0);
  });

  it("preserves capability and link state for the other supported actions", () => {
    renderMenu({ managed: false, canCopy: true, canDelete: false }, { hasLinks: false });

    expect(item("copy")?.hasAttribute("data-disabled")).toBe(false);
    expect(item("cut")?.hasAttribute("data-disabled")).toBe(true);
    expect(item("delete")?.hasAttribute("data-disabled")).toBe(true);
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
  capabilities: Capabilities,
  overrides: Partial<React.ComponentProps<typeof NodeContextMenu>> = {},
): void {
  act(() => {
    root.render(
      <NodeContextMenu
        position={{ x: 0, y: 0 }}
        capabilities={capabilities}
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
