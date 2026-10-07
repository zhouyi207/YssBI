import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const native = vi.hoisted(() => ({
  label: "main",
  onMoved: vi.fn<(listener: () => void) => Promise<() => void>>(),
  onResized: vi.fn<(listener: () => void) => Promise<() => void>>(),
  isMinimized: vi.fn<() => Promise<boolean>>(),
  isMaximized: vi.fn<() => Promise<boolean>>(),
  outerPosition: vi.fn<() => Promise<{ x: number; y: number }>>(),
  innerSize: vi.fn<() => Promise<{ width: number; height: number }>>(),
  unmaximize: vi.fn<() => Promise<void>>(),
  maximize: vi.fn<() => Promise<void>>(),
  center: vi.fn<() => Promise<void>>(),
  setSize: vi.fn<(size: { width: number; height: number }) => Promise<void>>(),
  setPosition: vi.fn<(position: { x: number; y: number }) => Promise<void>>(),
}));

vi.mock("@tauri-apps/api/core", () => ({ isTauri: () => true }));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => native,
  availableMonitors: async () => [
    { position: { x: 0, y: 0 }, size: { width: 1920, height: 1080 } },
  ],
}));

describe("main window geometry lifecycle", () => {
  let preferences: Map<string, string>;

  beforeEach(() => {
    vi.resetModules();
    vi.resetAllMocks();
    preferences = new Map();
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => preferences.get(key) ?? null,
      setItem: (key: string, value: string) => preferences.set(key, value),
    });
    vi.spyOn(console, "warn").mockImplementation(() => {});
    native.onMoved.mockResolvedValue(vi.fn());
    native.onResized.mockResolvedValue(vi.fn());
    native.isMinimized.mockResolvedValue(false);
    native.isMaximized.mockResolvedValue(false);
    native.outerPosition.mockResolvedValue({ x: 40, y: 80 });
    native.innerSize.mockResolvedValue({ width: 1100, height: 720 });
    native.unmaximize.mockResolvedValue(undefined);
    native.maximize.mockResolvedValue(undefined);
    native.center.mockResolvedValue(undefined);
    native.setSize.mockImplementation(async ({ width, height }) => {
      native.innerSize.mockResolvedValue({ width, height });
    });
    native.setPosition.mockImplementation(async ({ x, y }) => {
      native.outerPosition.mockResolvedValue({ x, y });
    });
  });

  afterEach(() => {
    vi.restoreAllMocks();
    vi.unstubAllGlobals();
  });

  it("releases a partial subscription before retrying native setup", async () => {
    const stopMoved = vi.fn();
    native.onMoved.mockResolvedValueOnce(stopMoved);
    native.onResized.mockRejectedValueOnce(new Error("resize subscription failed"));
    const { restoreMainWindowPage } = await import("./mainWindowGeometry");

    await restoreMainWindowPage("projects");
    expect(stopMoved).toHaveBeenCalledOnce();
    expect(native.setSize).not.toHaveBeenCalled();

    await restoreMainWindowPage("projects");
    await restoreMainWindowPage("editor");
    expect(native.onMoved).toHaveBeenCalledTimes(2);
    expect(native.onResized).toHaveBeenCalledTimes(2);
    expect(stopMoved).toHaveBeenCalledOnce();
    expect(native.setSize).toHaveBeenCalledTimes(2);
  });

  it("preserves page preferences after a partial restore and retries the same page", async () => {
    const editor = { x: 400, y: 300, width: 1400, height: 850, maximized: false };
    preferences.set("yssbi-main-window:editor", JSON.stringify(editor));
    const { restoreMainWindowPage } = await import("./mainWindowGeometry");
    await restoreMainWindowPage("projects");
    const projects = preferences.get("yssbi-main-window:projects");
    native.setPosition.mockRejectedValueOnce(new Error("window positioning failed"));

    await restoreMainWindowPage("editor");
    native.onMoved.mock.calls[0]![0]();
    await new Promise<void>((resolve) => setTimeout(resolve, 0));
    expect(preferences.get("yssbi-main-window:editor")).toBe(JSON.stringify(editor));
    expect(preferences.get("yssbi-main-window:projects")).toBe(projects);

    await restoreMainWindowPage("editor");
    expect(native.setSize).toHaveBeenCalledTimes(3);
    expect(native.setPosition).toHaveBeenCalledTimes(2);
    expect(preferences.get("yssbi-main-window:editor")).toBe(JSON.stringify(editor));

    await restoreMainWindowPage("projects");
    expect(preferences.get("yssbi-main-window:projects")).toBe(projects);
  });

  it("disposes pending native setup and ignores its queued restores and late callbacks", async () => {
    const stopMoved = vi.fn();
    const stopResized = vi.fn();
    let finishResize!: (stop: () => void) => void;
    native.onMoved.mockResolvedValueOnce(stopMoved);
    native.onResized.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finishResize = resolve;
        }),
    );
    const { restoreMainWindowPage, disposeMainWindowGeometryForHmr } =
      await import("./mainWindowGeometry");
    const first = restoreMainWindowPage("projects");
    const queued = restoreMainWindowPage("editor");
    await vi.waitFor(() => expect(native.onResized).toHaveBeenCalledOnce());
    disposeMainWindowGeometryForHmr();
    expect(stopMoved).toHaveBeenCalledOnce();
    finishResize(stopResized);
    await Promise.all([first, queued]);
    expect(stopResized).toHaveBeenCalledOnce();
    native.onMoved.mock.calls[0]![0]();
    native.onResized.mock.calls[0]![0]();
    await restoreMainWindowPage("projects");
    await Promise.resolve();
    disposeMainWindowGeometryForHmr();
    expect(stopMoved).toHaveBeenCalledOnce();
    expect(stopResized).toHaveBeenCalledOnce();
    expect(native.unmaximize).not.toHaveBeenCalled();
    expect(native.isMinimized).not.toHaveBeenCalled();
    expect(preferences.size).toBe(0);
  });
});
