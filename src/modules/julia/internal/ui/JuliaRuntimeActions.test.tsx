// @vitest-environment happy-dom

import { act } from "react";
import { createRoot, type Root } from "react-dom/client";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { TooltipProvider } from "@/components/ui/tooltip";

const mocks = vi.hoisted(() => ({
  openBayesWindow: vi.fn(),
  installJuliaRuntime: vi.fn(),
  needsInstall: true,
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
    needsInstall: mocks.needsInstall,
    label: "julia.worker.ready",
    tooltip: "julia.worker.readyDetail",
  }),
}));

vi.mock("@/features/application/julia/installJuliaRuntime", () => ({
  installJuliaRuntime: mocks.installJuliaRuntime,
}));

import { JuliaRuntimeActions } from "./JuliaRuntimeActions";

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe("JuliaRuntimeActions", () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
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
          <JuliaRuntimeActions />
        </TooltipProvider>,
      ),
    );
  }

  it("shows the Julia entry without a local installation preference", () => {
    render();
    expect(host.querySelector("[data-workbench-julia-action]")).not.toBeNull();
  });

  it("runs the actual installer when the backend reports a missing runtime", async () => {
    render();
    act(() => host.querySelector<HTMLButtonElement>("[data-workbench-julia-action]")?.click());
    const installButton = [...document.body.querySelectorAll("button")].find(
      (button) => button.textContent === "julia.install.confirm",
    );
    expect(installButton).not.toBeUndefined();
    await act(async () => {
      installButton?.click();
      await Promise.resolve();
    });
    expect(mocks.installJuliaRuntime).toHaveBeenCalledOnce();
  });
});
