// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";
import { usePluginStore } from "@/features/core/plugins/pluginStore";

const mocks = vi.hoisted(() => ({
  openBayesWindow: vi.fn(),
  installJuliaPlugin: vi.fn(),
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

vi.mock("@/features/application/window", () => ({
  openBayesWindow: mocks.openBayesWindow,
}));

vi.mock("@/features/application/statusBar/useJuliaWorkerStatus", () => ({
  useJuliaWorkerStatus: () => ({
    state: "ready",
    label: "julia.worker.ready",
    tooltip: "julia.worker.readyDetail",
  }),
}));

vi.mock("@/features/application/plugins/installJuliaPlugin", () => ({
  installJuliaPlugin: mocks.installJuliaPlugin,
}));

import { PluginActivityActions } from "./PluginActivityActions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("PluginActivityActions", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    localStorage.clear();
    usePluginStore.setState({ installedPluginIds: [] });
    vi.clearAllMocks();
    host = document.createElement("div");
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.replaceChildren();
  });

  function render(): void {
    act(() =>
      root.render(
        <TooltipProvider>
          <PluginActivityActions />
        </TooltipProvider>,
      ),
    );
  }

  it("keeps the extension manager slot visible and adds Julia after installation", () => {
    render();

    expect(host.querySelector("[data-workbench-plugin-manager]")).not.toBeNull();
    expect(host.querySelector('[data-workbench-plugin-slot="julia"]')).toBeNull();

    act(() => usePluginStore.getState().installPlugin("julia"));

    expect(host.querySelector('[data-workbench-plugin-slot="julia"]')).not.toBeNull();
  });

  it("routes the manager install action to the Julia installer", async () => {
    render();

    const managerButton = host.querySelector<HTMLButtonElement>("[data-workbench-plugin-manager]");
    act(() => managerButton?.click());

    const installButton = [...document.body.querySelectorAll("button")].find(
      (button) => button.textContent === "plugins.manager.install",
    );
    expect(installButton).not.toBeUndefined();

    act(() => installButton?.click());
    await act(async () => {
      await Promise.resolve();
    });

    expect(mocks.installJuliaPlugin).toHaveBeenCalledOnce();
  });

  it("filters the built-in catalog from the manager search field", () => {
    render();

    const managerButton = host.querySelector<HTMLButtonElement>("[data-workbench-plugin-manager]");
    act(() => managerButton?.click());

    const search = document.body.querySelector<HTMLInputElement>("[data-workbench-plugin-search]");
    expect(search).not.toBeNull();
    expect(document.body.querySelector('[data-workbench-plugin-card="julia"]')).not.toBeNull();

    act(() => {
      if (!search) return;
      const setNativeValue = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        "value",
      )?.set;
      setNativeValue?.call(search, "missing");
      search.dispatchEvent(
        new InputEvent("input", {
          bubbles: true,
          inputType: "insertText",
          data: "missing",
        }),
      );
    });

    expect(document.body.querySelector('[data-workbench-plugin-card="julia"]')).toBeNull();
    expect(document.body.textContent).toContain("plugins.manager.noResults");
  });
});
