// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { SidebarSectionEmptyState } from "./SidebarSectionEmptyState";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("Sidebar empty-state components", () => {
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

  it("renders a section state with the full accessible label", () => {
    act(() => {
      root.render(
        <TooltipProvider>
          <SidebarSectionEmptyState
            level={1}
            message="A deliberately long section empty-state message"
          />
        </TooltipProvider>,
      );
    });

    const message = host.querySelector(
      '[aria-label="A deliberately long section empty-state message"]',
    );
    expect(message).toBeInstanceOf(HTMLElement);
    expect((message as HTMLElement).tabIndex).toBe(0);

    act(() => (message as HTMLElement).focus());
    expect(document.activeElement).toBe(message);
  });
});
