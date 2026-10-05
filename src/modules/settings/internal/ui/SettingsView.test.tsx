// @vitest-environment happy-dom
import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { uiStore } from "@/features/core/ui/UIStore";
import { normalizeIpcError } from "@/services/ipc";
import { SettingsView } from "./SettingsView";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const settings = vi.hoisted(() => ({
  resetAllToDefaults: vi.fn(),
  resetAppearanceToDefaults: vi.fn(),
  updateAppearance: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  initReactI18next: { type: "3rdParty", init: vi.fn() },
  useTranslation: () => ({
    t: (key: string, values?: Record<string, unknown>) => {
      if (key === "common.error") return "Error";
      if (key === "common.incidentId") return "Incident ID";
      if (key === "common.unexpectedError") return "An unexpected error occurred";
      if (typeof values?.error === "string") return `${key}: ${values.error}`;
      return key;
    },
  }),
}));

vi.mock("./LanguageModelSettings", () => ({
  LanguageModelSettings: () => <p>settings.models.title</p>,
}));

vi.mock("@/app/i18n", () => ({
  i18n: { changeLanguage: vi.fn() },
}));

vi.mock("@/components/ui/scroll-area", () => ({
  ScrollArea: ({ children }: { children: unknown }) => children,
}));

vi.mock("@/shared/ui", () => ({
  Select: ({
    id,
    value,
    options,
    onChange,
    disabled,
  }: {
    id?: string;
    value: string;
    options: Array<{ label: string; value: string }>;
    onChange(value: string): void;
    disabled?: boolean;
  }) => (
    <select
      id={id}
      value={value}
      disabled={disabled}
      onChange={(event) => onChange(event.target.value)}
    >
      {options.map((option) => (
        <option key={option.value} value={option.value}>
          {option.label}
        </option>
      ))}
    </select>
  ),
}));

vi.mock("@/features/core/settings/settingsStore", () => {
  const state = {
    appearance: {
      colorTheme: "Dark Modern (Default)",
      language: "en-US",
      titleBarStyle: "custom",
      smoothScroll: true,
    },
    isLoading: false,
    updateAppearance: settings.updateAppearance,
    resetAllToDefaults: settings.resetAllToDefaults,
    resetAppearanceToDefaults: settings.resetAppearanceToDefaults,
  };
  const useSettingsStore = Object.assign(
    (selector: (value: typeof state) => unknown) => selector(state),
    {
      getState: () => state,
      subscribe: () => () => {},
    },
  );
  return { useSettingsStore };
});

function click(element: Element): void {
  act(() => element.dispatchEvent(new MouseEvent("click", { bubbles: true })));
}

describe("SettingsView preferences", () => {
  let host: HTMLDivElement;
  let root: Root;
  let modalId: string;

  beforeEach(() => {
    vi.clearAllMocks();
    settings.resetAllToDefaults.mockResolvedValue(undefined);
    settings.resetAppearanceToDefaults.mockResolvedValue(undefined);
    uiStore.showSettings();
    modalId = uiStore.getState().modals.find((modal) => modal.type === "settings")!.id;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    for (const modal of uiStore.getState().modals) uiStore.closeModal(modal.id);
    vi.restoreAllMocks();
  });

  function render(): void {
    act(() => root.render(<SettingsView modalId={modalId} />));
  }

  async function flushPromises(): Promise<void> {
    await act(async () => {
      await Promise.resolve();
      await Promise.resolve();
    });
  }

  async function openSection(section: string): Promise<void> {
    const button = [...host.querySelectorAll("button")].find(
      (item) => item.textContent === `settings.sections.${section}`,
    );
    if (!button) throw new Error(`${section} section button missing`);
    click(button);
    await act(async () => {
      await Promise.resolve();
    });
  }

  it("hides unrelated settings for an unmatched search and restores them when cleared", () => {
    render();
    const search = host.querySelector('input[aria-label="settings.searchPlaceholder"]')!;
    act(() => {
      Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value")!.set!.call(
        search,
        "no-matching-category",
      );
      search.dispatchEvent(new Event("input", { bubbles: true }));
    });
    expect(host.querySelector('input[type="password"]')).toBeNull();
    expect(host.querySelector('[role="status"]')?.textContent).toContain("settings.noResults");
    const clear = [...host.querySelectorAll("button")].find(
      (button) => button.textContent === "settings.clearSearch",
    )!;
    click(clear);
    expect(host.textContent).toContain("settings.models.title");
    expect(host.querySelector('[role="status"]')).toBeNull();
  });

  it("changes appearance colors only through the theme selector", async () => {
    render();
    expect(host.textContent).not.toContain("settings.sections.color");
    await openSection("appearance");

    const themeLabel = [...host.querySelectorAll("label")].find(
      (label) => label.textContent === "settings.labels.colorTheme",
    )!;
    const themeSelect = document.getElementById(themeLabel.htmlFor) as HTMLSelectElement;
    expect([...themeSelect.options].map((option) => option.value)).toEqual([
      "Dark Modern (Default)",
      "OLED Black",
      "Light Modern",
    ]);
    act(() => {
      themeSelect.value = "OLED Black";
      themeSelect.dispatchEvent(new Event("change", { bubbles: true }));
    });
    expect(settings.updateAppearance).toHaveBeenCalledWith({ colorTheme: "OLED Black" });
    expect(host.querySelector('input[type="color"]')).toBeNull();
  });

  it("updates smooth scrolling from the labeled preference switch", async () => {
    render();
    await openSection("appearance");
    const label = [...host.querySelectorAll("label")].find(
      (item) => item.textContent === "settings.labels.smoothScroll",
    )!;
    const control = document.getElementById(label.htmlFor)!;
    expect(control.getAttribute("role")).toBe("switch");
    expect(control.getAttribute("aria-checked")).toBe("true");
    click(control);
    expect(settings.updateAppearance).toHaveBeenCalledWith({ smoothScroll: false });
  });

  it("shows a section reset failure with the active section", async () => {
    vi.spyOn(uiStore, "confirm").mockResolvedValue(true);
    settings.resetAppearanceToDefaults.mockRejectedValueOnce(
      normalizeIpcError("reset_appearance_settings", {
        code: "settings_section_reset_failed",
        details: null,
        incidentId: null,
      }),
    );
    render();

    await openSection("appearance");
    const resetSection = [...host.querySelectorAll("button")].find(
      (item) => item.textContent === "common.restoreDefaults",
    );
    click(resetSection!);
    await flushPromises();

    const alert = host.querySelector<HTMLElement>("main [data-settings-section-reset-error]");
    expect(alert?.textContent).toContain("settings_section_reset_failed");
  });

  it("does not show feedback after a successful reset", async () => {
    vi.spyOn(uiStore, "confirm").mockResolvedValue(true);
    render();

    const resetAll = [...host.querySelectorAll("button")].find(
      (item) => item.textContent === "settings.restorePreferences",
    );
    click(resetAll!);
    await flushPromises();

    expect(host.querySelector("[data-settings-reset-all-error]")).toBeNull();
  });

  it.each([
    ["settings.restorePreferences", "resetAllToDefaults"],
    ["common.restoreDefaults", "resetAppearanceToDefaults"],
  ] as const)(
    "keeps %s pending until confirmed and leaves settings intact on cancel",
    async (label, action) => {
      render();
      await openSection("appearance");
      const button = [...host.querySelectorAll("button")].find(
        (item) => item.textContent === label,
      )!;

      click(button);
      await flushPromises();
      expect(settings[action]).not.toHaveBeenCalled();
      const pendingCancel = uiStore.getState().modals;
      const canceled = pendingCancel[pendingCancel.length - 1];
      if (canceled.type !== "confirm") throw new Error("Expected a reset confirmation");
      canceled.options.onCancel?.();
      uiStore.closeModal(canceled.id);
      await flushPromises();
      expect(settings[action]).not.toHaveBeenCalled();
      expect(settings.updateAppearance).not.toHaveBeenCalled();

      click(button);
      const pendingConfirm = uiStore.getState().modals;
      const confirmed = pendingConfirm[pendingConfirm.length - 1];
      if (confirmed.type !== "confirm") throw new Error("Expected a reset confirmation");
      confirmed.options.onConfirm();
      uiStore.closeModal(confirmed.id);
      await flushPromises();
      expect(settings[action]).toHaveBeenCalledOnce();
    },
  );
});
