// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TFunction } from "i18next";

const runtime = vi.hoisted(() => ({
  install: vi.fn(),
}));
const ui = vi.hoisted(() => ({
  confirm: vi.fn(),
  alert: vi.fn(),
  startProgress: vi.fn(),
  finishProgress: vi.fn(),
}));

vi.mock("@/services/julia/juliaRuntimeService", () => ({
  JuliaRuntimeService: {
    install: runtime.install,
  },
}));

vi.mock("@/features/core/ui/UIStore", () => ({
  uiStore: ui,
}));

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { installJuliaRuntime } from "./installJuliaRuntime";

const translate = ((key: string) => key) as unknown as TFunction;

describe("installJuliaRuntime", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    ui.confirm.mockResolvedValue(true);
  });

  it("succeeds only after the managed runtime is ready", async () => {
    runtime.install.mockResolvedValue({
      state: "ready",
      version: "1.12.0",
      installDir: null,
    });

    await expect(installJuliaRuntime(translate)).resolves.toBe(true);

    expect(ui.startProgress).toHaveBeenCalledOnce();
    expect(ui.finishProgress).toHaveBeenCalledOnce();
    expect(ui.alert).not.toHaveBeenCalled();
  });

  it("reports an invalid runtime installation", async () => {
    runtime.install.mockResolvedValue({
      state: "invalid",
      version: null,
      installDir: null,
    });
    ui.alert.mockResolvedValue(undefined);

    await expect(installJuliaRuntime(translate)).resolves.toBe(false);

    expect(ui.alert).toHaveBeenCalledOnce();
  });
});
