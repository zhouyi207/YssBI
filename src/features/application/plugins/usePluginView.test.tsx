// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { afterEach, expect, it, vi } from "vitest";
import { usePluginView } from "./usePluginView";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";

const mocked = vi.hoisted(() => ({
  attach: vi.fn(),
  detach: vi.fn(async () => {}),
  call: vi.fn(),
  fulfill: vi.fn(),
  theme: { mode: "dark" },
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ i18n: { language: "en-US" } }) }));
vi.mock("@/features/application/settings/applicationSettings", () => ({
  useApplicationSettings: () => ({ theme: mocked.theme }),
}));
vi.mock("@/features/application/project/projectIOStore", () => ({
  useProjectIOStore: () => undefined,
}));
vi.mock("@/services/plugins/pluginService", () => ({ pluginService: mocked }));
vi.mock("@/features/application/plugins/pluginActions", () => ({
  fulfillPluginUiRequest: mocked.fulfill,
}));
afterEach(() => {
  vi.unstubAllGlobals();
  vi.clearAllMocks();
});

it("keeps visibility changes connected and requires a fresh explicit retry after navigation", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const channels: {
    port1: {
      close: ReturnType<typeof vi.fn>;
      postMessage: ReturnType<typeof vi.fn>;
      onmessage?: (event: { data: unknown }) => Promise<void>;
    };
    port2: object;
  }[] = [];
  vi.stubGlobal(
    "MessageChannel",
    class {
      port1 = { close: vi.fn(), postMessage: vi.fn() };
      port2 = { close: vi.fn() };
      constructor() {
        channels.push(this);
      }
    },
  );
  let resolveLate!: (value: unknown) => void;
  mocked.call.mockImplementation(
    () =>
      new Promise((resolve) => {
        resolveLate = resolve;
      }),
  );
  mocked.attach.mockImplementation(async () => ({
    sessionId: `session-${mocked.attach.mock.calls.length}`,
    html: "<p>verified</p>",
    installationGeneration: "1",
  }));
  const plugin = {
    manifest: {
      id: "example.ui",
      contributes: { views: [{ id: "runtime", scope: "application" }] },
    },
    installationGeneration: "1",
    enabled: true,
  } as InstalledPlugin;
  let current!: ReturnType<typeof usePluginView>;
  function Harness({ visible = true }: { visible?: boolean }) {
    current = usePluginView({ plugin, viewId: "runtime", onOpen: vi.fn(), visible });
    return <span>{current.session?.sessionId}</span>;
  }
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  try {
    await act(async () => root.render(<Harness />));
    act(() => current.onLoad());
    for (let step = 0; step < 12; step++)
      await act(async () => root.render(<Harness visible={step % 2 === 0} />));
    expect(mocked.attach).toHaveBeenCalledOnce();
    expect(mocked.detach).not.toHaveBeenCalled();
    expect(current.error).toBeNull();
    const late = channels[0].port1.onmessage!({
      data: { id: "late", method: "system.save_file", input: null },
    });
    await act(async () => current.onLoad());
    expect(mocked.detach).toHaveBeenCalledWith("session-1");
    expect(channels[0].port1.close).toHaveBeenCalled();
    expect(current.error).toMatchObject({ phase: "navigation", code: "plugin_view_navigation" });
    expect(mocked.attach).toHaveBeenCalledOnce();
    await act(async () => current.retry());
    expect(current.session?.sessionId).toBe("session-2");
    expect(current.error).toBeNull();
    resolveLate({ hostUi: { kind: "saveFile" } });
    await late;
    expect(mocked.fulfill).not.toHaveBeenCalled();
  } finally {
    await act(async () => root.unmount());
    container.remove();
  }
});
