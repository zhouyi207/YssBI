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
  resetAiToDefaults: vi.fn(),
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
    ai: {
      openAiModel: "",
      openAiBaseUrl: "https://api.openai.com/v1",
      openAiApiKey: "",
    },
    appearance: {
      colorTheme: "Dark Modern (Default)",
      language: "en-US",
      titleBarStyle: "custom",
      smoothScroll: true,
    },
    isLoading: false,
    updateAi: vi.fn(),
    updateAppearance: settings.updateAppearance,
    resetAllToDefaults: settings.resetAllToDefaults,
    resetAiToDefaults: settings.resetAiToDefaults,
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
  const onRequestClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    settings.resetAllToDefaults.mockResolvedValue(undefined);
    settings.resetAiToDefaults.mockResolvedValue(undefined);
    settings.resetAppearanceToDefaults.mockResolvedValue(undefined);
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    while (uiStore.getState().modals.length) uiStore.closeModal();
    vi.restoreAllMocks();
  });

  function render(): void {
    act(() => root.render(<SettingsView onRequestClose={onRequestClose} />));
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

  it("exposes OpenAI model and API key controls in the AI section", async () => {
    render();
    await openSection("ai");

    expect(host.textContent).toContain("settings.labels.openAiModel");
    expect(host.textContent).toContain("settings.labels.openAiBaseUrl");
    expect(host.textContent).toContain("settings.labels.openAiApiKey");
    expect(host.querySelector('input[type="password"]')).not.toBeNull();
  });

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
    expect(host.querySelector('input[type="password"]')).not.toBeNull();
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

  it("shows an IPC reset-all failure in a top-level alert without raw backend details", async () => {
    vi.spyOn(uiStore, "confirm").mockResolvedValue(true);
    settings.resetAllToDefaults.mockRejectedValueOnce(
      normalizeIpcError("reset_all_settings", {
        code: "settings_reset_failed",
        details: { debug: "raw backend settings failure" },
        incidentId: "incident-settings-all-42",
      }),
    );
    render();

    const resetAll = [...host.querySelectorAll("button")].find(
      (item) => item.textContent === "common.restoreAllDefaults",
    );
    click(resetAll!);
    await flushPromises();

    const alert = host.querySelector<HTMLElement>("[data-settings-reset-all-error]");
    expect(alert?.textContent).toContain("settings_reset_failed");
    expect(alert?.textContent).toContain("incident-settings-all-42");
    expect(alert?.textContent).not.toContain("raw backend settings failure");
  });

  it("shows a section reset failure with the active section", async () => {
    vi.spyOn(uiStore, "confirm").mockResolvedValue(true);
    settings.resetAiToDefaults.mockRejectedValueOnce(
      normalizeIpcError("reset_ai_settings", {
        code: "settings_section_reset_failed",
        details: null,
        incidentId: null,
      }),
    );
    render();

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
      (item) => item.textContent === "common.restoreAllDefaults",
    );
    click(resetAll!);
    await flushPromises();

    expect(host.querySelector("[data-settings-reset-all-error]")).toBeNull();
  });

  it.each([
    ["common.restoreAllDefaults", "resetAllToDefaults"],
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

  it("closes after switching between immediately applied preference sections", async () => {
    const confirm = vi.spyOn(uiStore, "confirm").mockResolvedValue(false);
    render();
    await openSection("appearance");
    await openSection("ai");

    click(host.querySelector('button[aria-label="settings.close"]')!);
    expect(onRequestClose).toHaveBeenCalledOnce();
    expect(confirm).not.toHaveBeenCalled();
  });
});
