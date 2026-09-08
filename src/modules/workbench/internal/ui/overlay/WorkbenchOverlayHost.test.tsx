// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it } from "vitest";
import { DialogDescription, DialogTitle } from "@/components/ui/dialog";
import { workbenchUi } from "../../state/ui";
import { WorkbenchOverlayHost } from "./WorkbenchOverlayHost";
import type { WorkbenchOverlayRegistry } from "./overlayContribution";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const overlays: WorkbenchOverlayRegistry = {
  settings: ({ onRequestClose }) => (
    <>
      <DialogTitle>Settings</DialogTitle>
      <DialogDescription>Application preferences</DialogDescription>
      <input aria-label="Setting value" />
      <button onClick={onRequestClose}>Close settings</button>
    </>
  ),
  nodeDocumentation: () => null,
};

function click(element: Element | null): void {
  if (!element) throw new Error("missing test element");
  act(() => {
    element.dispatchEvent(
      new PointerEvent("pointerdown", { bubbles: true, cancelable: true, pointerType: "mouse" }),
    );
    element.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "mouse" }));
    element.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
  });
}

describe("WorkbenchOverlayHost settings dismissal", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    workbenchUi.setSettingsOpen(false);
    workbenchUi.setNodeDocumentationOpen(false);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    workbenchUi.setSettingsOpen(false);
  });

  it("closes settings from the non-draggable backdrop while keeping content interactions inside", async () => {
    act(() => {
      workbenchUi.openSettings();
      root.render(<WorkbenchOverlayHost overlays={overlays} />);
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 0));
    });

    const overlay = document.querySelector('[data-slot="dialog-overlay"]');
    expect(overlay?.hasAttribute("data-tauri-drag-region")).toBe(false);
    expect(overlay?.classList.contains("cursor-move")).toBe(false);

    click(document.querySelector('input[aria-label="Setting value"]'));
    expect(workbenchUi.getSnapshot().isSettingsOpen).toBe(true);

    click(overlay);
    expect(workbenchUi.getSnapshot().isSettingsOpen).toBe(false);
    expect(document.querySelector('[role="dialog"]')).toBeNull();
  });
});
