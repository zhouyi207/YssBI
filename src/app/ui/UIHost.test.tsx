// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { TooltipProvider } from "@/components/ui/tooltip";
import { triggerImportData } from "@/features/application/dataManagement/useDatabaseManagement";
import { uiStore } from "@/features/core/ui/UIStore";
import { UIHost } from "./UIHost";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({ t: (key: string) => key }),
}));

function getButton(container: ParentNode, label: string): HTMLButtonElement {
  const button = [...container.querySelectorAll("button")].find((candidate) =>
    candidate.textContent?.includes(label),
  );
  if (!button) throw new Error(`missing button: ${label}`);
  return button;
}

async function clickButton(container: ParentNode, label: string) {
  await act(async () => {
    const button = getButton(container, label);
    button.focus();
    button.click();
  });
  await flushDialogEffects();
}

async function flushDialogEffects() {
  await act(async () => {
    await new Promise((resolve) => setTimeout(resolve, 0));
  });
}

describe("UIHost modal stack", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(async () => {
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
    await act(async () => {
      root.render(
        <TooltipProvider>
          <UIHost />
        </TooltipProvider>,
      );
      triggerImportData();
    });
    await flushDialogEffects();
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    while (uiStore.getState().modals.length) uiStore.closeModal();
    host.remove();
    await flushDialogEffects();
  });

  it("keeps the import dialog and its category when a connection dialog is canceled", async () => {
    const parent = document.querySelector<HTMLElement>('[role="dialog"]')!;
    await clickButton(parent, "importModal.categories.sql");
    const opener = getButton(parent, "importModal.types.postgres.label");
    await clickButton(parent, "importModal.types.postgres.label");

    const dialogs = [...document.querySelectorAll<HTMLElement>('[role="dialog"]')];
    expect(dialogs).toHaveLength(2);
    expect(dialogs[0]).toBe(parent);
    expect(uiStore.getState().modals.map((modal) => modal.type)).toEqual([
      "import",
      "sqlConnection",
    ]);
    const child = dialogs[1];
    const overlays = [...document.querySelectorAll<HTMLElement>('[data-slot="dialog-overlay"]')];
    expect(Number(overlays[1].style.zIndex)).toBeGreaterThan(Number(parent.style.zIndex));
    expect(Number(child.style.zIndex)).toBeGreaterThan(Number(overlays[1].style.zIndex));
    expect(parent.style.pointerEvents).toBe("none");
    expect(child.contains(document.activeElement)).toBe(true);

    await clickButton(child, "common.cancel");

    expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
    expect(document.querySelector('[role="dialog"]')).toBe(parent);
    expect(parent.querySelector('[aria-current="page"]')?.textContent).toContain(
      "importModal.categories.sql",
    );
    expect(document.activeElement).toBe(opener);
    expect(uiStore.getState().modals.map((modal) => modal.type)).toEqual(["import"]);
  });

  it("dismisses only the top dialog and preserves the form beneath it", async () => {
    const parent = document.querySelector<HTMLElement>('[role="dialog"]')!;
    await clickButton(parent, "importModal.categories.sql");
    await clickButton(parent, "importModal.types.postgres.label");
    const connection = document.querySelectorAll<HTMLElement>('[role="dialog"]')[1];
    expect(connection).toBeDefined();
    const input = connection.querySelector("input")!;
    act(() => {
      input.focus();
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(
        input,
        "db.example.test",
      );
      input.dispatchEvent(new Event("input", { bubbles: true }));
    });
    let dismissed!: Promise<void>;
    await act(async () => {
      dismissed = uiStore.alert({
        title: "Connection feedback",
        message: "Try again",
        closeText: "Close",
        type: "warning",
      });
    });
    await flushDialogEffects();
    expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(3);

    await act(async () => {
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
      );
    });
    await dismissed;
    await flushDialogEffects();

    expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(2);
    expect(connection.querySelector("input")).toBe(input);
    expect(input.value).toBe("db.example.test");
    expect(document.activeElement).toBe(input);

    const overlay = document.querySelectorAll('[data-slot="dialog-overlay"]')[1];
    await act(async () => {
      overlay.dispatchEvent(
        new PointerEvent("pointerdown", { bubbles: true, cancelable: true, pointerType: "mouse" }),
      );
      overlay.dispatchEvent(new PointerEvent("pointerup", { bubbles: true, pointerType: "mouse" }));
      overlay.dispatchEvent(new MouseEvent("click", { bubbles: true, cancelable: true }));
    });
    await flushDialogEffects();

    expect(document.querySelectorAll('[role="dialog"]')).toHaveLength(1);
    expect(document.querySelector('[role="dialog"]')).toBe(parent);
    await act(async () => {
      document.activeElement?.dispatchEvent(
        new KeyboardEvent("keydown", { key: "Escape", bubbles: true, cancelable: true }),
      );
    });
    expect(uiStore.getState().modals).toHaveLength(0);
  });
});
