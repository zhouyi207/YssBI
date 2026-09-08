// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { PluginProvider, usePlugins } from "./PluginProvider";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";

const mocks = vi.hoisted(() => ({
  list: vi.fn(),
  uninstall: vi.fn(),
  confirm: vi.fn(),
  sync: vi.fn(),
  open: vi.fn(),
  t: (key: string) => key,
}));
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: mocks.t }) }));
vi.mock("@/services/plugins/pluginService", () => ({
  pluginService: { list: mocks.list, uninstall: mocks.uninstall },
}));
vi.mock("@/features/core/ui/ui", () => ({ ui: { confirm: mocks.confirm } }));
vi.mock("@/services/platform/pathDialog", () => ({
  openPathDialog: vi.fn(),
  savePathDialog: vi.fn(),
}));
vi.mock("@/services/platform/opener", () => ({ revealPath: vi.fn() }));
vi.mock("@/modules/workbench/public", () => ({
  syncPluginWorkbenchViews: mocks.sync,
  openPluginWorkbenchView: mocks.open,
}));

function installed(id: string, enabled = true): InstalledPlugin {
  return {
    enabled,
    installationGeneration: "1",
    packageDigest: "digest",
    processState: "stopped",
    manifest: {
      id,
      name: id,
      publisher: "example",
      version: "1.0.0",
      description: "",
      schemaVersion: 1,
      hostApi: "^1",
      target: "test",
      executable: "bin/app",
      execution: "trustedNative",
      protocol: { major: 1, minMinor: 0, maxMinor: 0, requiredFeatures: [] },
      permissions: [],
      uiMethods: [],
      resourceBudget: {
        frameBytes: 1024,
        pendingRequests: 1,
        activeTasks: 1,
        queuedBytes: 1024,
        snapshotBytes: 1024,
        privateStorageBytes: 1024,
        views: 2,
      },
      contributes: {
        commands: [],
        taskTypes: [],
        views: [
          {
            id: "runtime",
            title: "Runtime",
            entry: "web/index.html",
            scope: "application",
            location: "sidebar",
          },
          {
            id: "analysis",
            title: "Analysis",
            entry: "web/index.html",
            scope: "project",
            location: "editor",
          },
        ],
      },
    },
  };
}

it("closes only after confirmed uninstall and cannot restore panels from a late registry response", async () => {
  vi.stubGlobal("IS_REACT_ACT_ENVIRONMENT", true);
  const removed = installed("example.removed");
  const kept = installed("example.kept", false);
  const initial = [removed, kept];
  mocks.list.mockRejectedValueOnce(new Error("registry unavailable"));
  let current!: ReturnType<typeof usePlugins>;
  function Consumer() {
    current = usePlugins();
    return null;
  }
  const container = document.createElement("div");
  document.body.append(container);
  const root = createRoot(container);
  try {
    await act(async () =>
      root.render(
        <PluginProvider>
          <Consumer />
        </PluginProvider>,
      ),
    );
    expect(mocks.sync).not.toHaveBeenCalled();
    mocks.list.mockResolvedValue(initial);
    await act(async () => current.refresh());
    current.open(removed.manifest.id, removed.manifest.contributes.views[0]);
    const isOpenCurrent = mocks.open.mock.calls[0][2] as () => boolean;
    expect(isOpenCurrent()).toBe(true);

    mocks.confirm.mockResolvedValue(false);
    await act(async () => current.uninstall(removed));
    expect(mocks.uninstall).not.toHaveBeenCalled();
    expect(mocks.sync.mock.lastCall?.[0]).toEqual([removed.manifest.id, kept.manifest.id]);
    const beforeFailure = mocks.sync.mock.calls.length;
    mocks.confirm.mockResolvedValue(true);
    mocks.uninstall.mockRejectedValueOnce(new Error("plugin busy"));
    await act(async () => current.uninstall(removed));
    expect(mocks.sync).toHaveBeenCalledTimes(beforeFailure);
    expect(current.plugins).toEqual(initial);

    let resolveOldList!: (plugins: InstalledPlugin[]) => void;
    mocks.list.mockImplementationOnce(
      () =>
        new Promise<InstalledPlugin[]>((resolve) => {
          resolveOldList = resolve;
        }),
    );
    let oldRefresh!: Promise<void>;
    act(() => {
      oldRefresh = current.refresh();
    });
    let confirmRemoval!: () => void;
    mocks.uninstall.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          confirmRemoval = resolve;
        }),
    );
    let uninstalling!: Promise<void>;
    await act(async () => {
      uninstalling = current.uninstall(removed);
    });
    expect(current.plugins).toEqual(initial);
    expect(isOpenCurrent()).toBe(true);
    mocks.list.mockRejectedValueOnce(new Error("post-commit query unavailable"));
    await act(async () => {
      confirmRemoval();
      await uninstalling;
    });
    expect(current.plugins).toEqual([kept]);
    expect(mocks.sync.mock.lastCall?.[0]).toEqual([kept.manifest.id]);
    expect(mocks.sync.mock.lastCall?.[1]).toEqual([]);
    expect(isOpenCurrent()).toBe(false);
    const afterRemoval = mocks.sync.mock.calls.length;
    await act(async () => {
      resolveOldList(initial);
      await oldRefresh;
    });
    expect(mocks.sync).toHaveBeenCalledTimes(afterRemoval);
    expect(current.plugins).toEqual([kept]);
    current.open(removed.manifest.id, removed.manifest.contributes.views[1]);
    expect(mocks.open).toHaveBeenCalledOnce();
  } finally {
    await act(async () => root.unmount());
    container.remove();
    vi.unstubAllGlobals();
  }
});
