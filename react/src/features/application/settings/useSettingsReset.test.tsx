// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { uiStore } from "@/features/core/ui/UIStore";
import { useSettingsReset } from "./useSettingsReset";

const settings = vi.hoisted(() => ({
  resetAllToDefaults: vi.fn(async () => {}),
  resetAppearanceToDefaults: vi.fn(async () => {}),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock("@/features/core/settings/ui", () => ({ settingsUi: settings }));
vi.mock("@/features/application/userErrorSummary", () => ({
  formatInlineUserError: () => "reset failed",
}));

let root: Root;
let host: HTMLDivElement;
let modalId: string;
let current: ReturnType<typeof useSettingsReset>;

function Harness() {
  current = useSettingsReset(modalId);
  return null;
}

beforeEach(() => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  vi.clearAllMocks();
  uiStore.showSettings();
  modalId = uiStore.getState().modals.find((modal) => modal.type === "settings")!.id;
  host = document.createElement("div");
  root = createRoot(host);
  act(() => root.render(<Harness />));
});

afterEach(async () => {
  await act(async () => {
    root.unmount();
    for (const modal of uiStore.getState().modals) uiStore.closeModal(modal.id);
  });
  vi.unstubAllGlobals();
});

it("admits one reset through confirmation and persistence, then releases it on cancellation", async () => {
  let first!: Promise<void>;
  let duplicate!: Promise<void>;
  act(() => {
    first = current.resetSettings("all");
    duplicate = current.resetSettings("appearance");
  });
  const confirmations = uiStore.getState().modals.filter((modal) => modal.type === "confirm");
  expect(confirmations).toHaveLength(1);
  expect(confirmations[0].parentId).toBe(modalId);
  await act(async () => {
    uiStore.closeModal(confirmations[0].id);
    await Promise.all([first, duplicate]);
  });
  expect(settings.resetAllToDefaults).not.toHaveBeenCalled();
  expect(settings.resetAppearanceToDefaults).not.toHaveBeenCalled();
  expect(current.isResetPending).toBe(false);

  let finishSave!: () => void;
  settings.resetAppearanceToDefaults.mockImplementationOnce(
    () =>
      new Promise<void>((resolve) => {
        finishSave = resolve;
      }),
  );
  let retry!: Promise<void>;
  act(() => {
    retry = current.resetSettings("appearance");
  });
  const confirmed = uiStore.getState().modals.find((modal) => modal.type === "confirm")!;
  await act(async () => {
    confirmed.options.onConfirm();
    uiStore.closeModal(confirmed.id);
  });
  expect(settings.resetAppearanceToDefaults).toHaveBeenCalledOnce();
  expect(current.isResetting).toBe(true);
  await act(async () => {
    await current.resetSettings("all");
  });
  expect(uiStore.getState().modals.some((modal) => modal.type === "confirm")).toBe(false);
  await act(async () => {
    finishSave();
    await retry;
  });
  expect(current.isResetPending).toBe(false);
});

it("does not persist a confirmed reset after its settings page has closed", async () => {
  let reset!: Promise<void>;
  act(() => {
    reset = current.resetSettings("all");
  });
  const confirmation = uiStore.getState().modals.find((modal) => modal.type === "confirm")!;
  act(() => {
    confirmation.options.onConfirm();
    root.render(null);
    uiStore.closeModal(modalId);
  });
  await act(async () => {
    await reset;
  });
  expect(settings.resetAllToDefaults).not.toHaveBeenCalled();
  expect(uiStore.getState().modals).toEqual([]);
});
