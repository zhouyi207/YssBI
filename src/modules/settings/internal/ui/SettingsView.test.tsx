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
  resetThemeToDefaults: vi.fn(),
  resetEditorToDefaults: vi.fn(),
  resetAppearanceToDefaults: vi.fn(),
}));

const computation = vi.hoisted(() => ({
  enabled: true,
  confirmed: { settingsRevision: 3 },
  draft: { absolute: "1e-12", relative: "1e-9", statistics: "listwise" as const },
  isLoading: false,
  isApplying: false,
  isDirty: false,
  validationError: null as string | null,
  error: null as string | null,
  setDraft: vi.fn(),
  apply: vi.fn(async () => undefined),
  restoreRecommended: vi.fn(),
}));

vi.mock("@/features/application/projectSettings/useProjectComputationSettings", () => ({
  useProjectComputationSettings: () => computation,
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
    theme: {
      mode: "dark",
      workbenchBackground: "#000000",
      sidebarBackground: "#000000",
      nodeBackground: "#000000",
      foreground: "#ffffff",
      mutedForeground: "#999999",
      accentColor: "#000000",
      borderColor: "#333333",
      gridColor: "#222222",
      selectionColor: "#000000",
    },
    editor: { showGrid: true, autoSave: false, snapToGrid: true, fontSize: 12 },
    appearance: {
      colorTheme: "Dark Modern (Default)",
      language: "en-US",
      titleBarStyle: "custom",
      smoothScroll: true,
    },
    project: { projectName: "", exportPath: "" },
    isLoading: false,
    updateTheme: vi.fn(),
    updateEditor: vi.fn(),
    updateAppearance: vi.fn(),
    updateProject: vi.fn(),
    resetAllToDefaults: settings.resetAllToDefaults,
    resetThemeToDefaults: settings.resetThemeToDefaults,
    resetEditorToDefaults: settings.resetEditorToDefaults,
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

describe("SettingsView computation settings", () => {
  let host: HTMLDivElement;
  let root: Root;
  const onRequestClose = vi.fn();

  beforeEach(() => {
    vi.clearAllMocks();
    settings.resetAllToDefaults.mockResolvedValue(undefined);
    settings.resetThemeToDefaults.mockResolvedValue(undefined);
    settings.resetEditorToDefaults.mockResolvedValue(undefined);
    settings.resetAppearanceToDefaults.mockResolvedValue(undefined);
    computation.enabled = true;
    computation.isDirty = false;
    computation.validationError = null;
    computation.error = null;
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
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

  async function openComputation(): Promise<void> {
    await openSection("computation");
  }

  it("disables the computation group when no project is open", async () => {
    computation.enabled = false;
    render();
    await openComputation();
    const group = host.querySelector(
      '[role="group"][aria-label="settings.computation.groupLabel"]',
    );
    expect(group?.getAttribute("aria-disabled")).toBe("true");
    expect(
      group?.querySelectorAll("input:disabled, select:disabled, button:disabled").length,
    ).toBeGreaterThan(0);
  });

  it("does not offer the removed panel-position appearance setting", async () => {
    render();
    await openSection("appearance");

    expect(host.textContent).not.toContain("settings.labels.panelPosition");
    expect(host.textContent).not.toContain("settings.descriptions.panelPosition");
  });

  it("exposes only semantic color controls and keeps pin colors fixed", async () => {
    render();
    await openSection("color");

    expect(host.textContent).toContain("settings.labels.workbenchBackground");
    expect(host.textContent).toContain("settings.labels.nodeBackground");
    expect(host.textContent).toContain("settings.labels.foreground");
    expect(host.textContent).toContain("settings.labels.mutedForeground");
    expect(host.textContent).toContain("settings.labels.borderColor");
    expect(host.textContent).toContain("settings.labels.gridColor");
    expect(host.textContent).toContain("settings.labels.selectionColor");
    expect(host.textContent).not.toContain("settings.groups.pinColors");
    expect(host.textContent).not.toContain("settings.labels.executionColor");
    expect(host.textContent).not.toContain("settings.labels.int32Color");
    expect(host.querySelectorAll('input[type="color"]').length).toBe(9);
  });

  it("shows tolerance formula help, Listwise/Reject, Apply, and recommended reset", async () => {
    computation.isDirty = true;
    vi.spyOn(uiStore, "confirm").mockResolvedValue(true);
    render();
    await openComputation();
    expect(host.textContent).toContain("|a - b| ≤ max(absolute, relative × max(|a|, |b|))");
    expect(host.textContent).toContain("Listwise");
    expect(host.textContent).toContain("Reject");
    click(
      [...host.querySelectorAll("button")].find(
        (item) => item.textContent === "Restore Recommended Values",
      )!,
    );
    click([...host.querySelectorAll("button")].find((item) => item.textContent === "Apply")!);
    expect(computation.restoreRecommended).toHaveBeenCalledOnce();
    expect(computation.apply).toHaveBeenCalledOnce();
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
    settings.resetEditorToDefaults.mockRejectedValueOnce(
      normalizeIpcError("reset_editor_settings", {
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

  it("uses the application confirmation modal before dirty close and section changes", async () => {
    computation.isDirty = true;
    const confirm = vi.spyOn(uiStore, "confirm").mockResolvedValue(false);
    render();
    await openComputation();

    click(host.querySelector('button[aria-label="Close settings"]')!);
    await act(async () => {
      await Promise.resolve();
    });
    expect(confirm).toHaveBeenCalledWith(
      expect.objectContaining({ title: "Discard computation changes?" }),
    );
    expect(onRequestClose).not.toHaveBeenCalled();

    click(
      [...host.querySelectorAll("button")].find(
        (item) => item.textContent === "settings.sections.editor",
      )!,
    );
    await act(async () => {
      await Promise.resolve();
    });
    expect(confirm).toHaveBeenCalledTimes(2);
    expect(host.textContent).toContain("settings.sections.computation");
  });
});
