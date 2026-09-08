// @vitest-environment happy-dom
import { act } from "react";
import { createRoot } from "react-dom/client";
import { expect, it, vi } from "vitest";
import { PluginsPanel } from "./PluginsPanel";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";
vi.mock("react-i18next", () => ({ useTranslation: () => ({ t: (key: string) => key }) }));
vi.mock("@/modules/workbench/public", () => ({
  ActivityPanelShell: ({ children }: { children: React.ReactNode }) => children,
}));
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
it("renders all installed plugins without search and preserves group collapse and actions", () => {
  const host = document.createElement("div");
  document.body.appendChild(host);
  const root = createRoot(host);
  const open = vi.fn();
  const install = vi.fn();
  const plugin: InstalledPlugin = {
    enabled: true,
    installationGeneration: "1",
    packageDigest: "digest",
    processState: "stopped",
    manifest: {
      id: "example.statistics",
      name: "Statistics",
      description: "Compute locally",
      publisher: "example",
      version: "1.0.0",
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
        views: 1,
      },
      contributes: { commands: [], taskTypes: [], views: [] },
    },
  };
  const render = (plugins: InstalledPlugin[]) => {
    act(() =>
      root.render(
        <PluginsPanel
          plugins={plugins}
          loading={false}
          busy={false}
          error={null}
          onRefresh={vi.fn()}
          onInstall={install}
          onOpen={open}
          onToggle={vi.fn()}
          onUninstall={vi.fn()}
        />,
      ),
    );
  };
  const installedGroup = () => host.querySelector("button[aria-expanded]")!.parentElement!;
  try {
    render([]);
    expect(installedGroup().children).toHaveLength(1);
    expect(host.querySelector('[role="status"]')).toBeNull();
    const installButton = host.querySelector<HTMLButtonElement>(
      'header button[aria-label="plugins.installPackage"]',
    )!;
    expect(installButton).not.toBeNull();
    act(() => installButton.click());
    expect(install).toHaveBeenCalledOnce();

    const otherPlugin: InstalledPlugin = {
      ...plugin,
      enabled: false,
      manifest: { ...plugin.manifest, id: "example.other", name: "Other extension" },
    };
    render([plugin, otherPlugin]);
    expect(host.querySelector("input")).toBeNull();
    expect(
      [...host.querySelectorAll("[data-plugin-item]")].map((item) =>
        item.getAttribute("data-plugin-item"),
      ),
    ).toEqual([plugin.manifest.id, otherPlugin.manifest.id]);
    const groupToggle = host.querySelector<HTMLButtonElement>("button[aria-expanded]")!;
    expect(groupToggle.lastElementChild?.textContent).toBe("2");
    expect(host.querySelector("article details")).toBeNull();
    act(() =>
      [...host.querySelectorAll("button")]
        .find((button) => button.textContent === "Statistics")!
        .click(),
    );
    expect(open).toHaveBeenCalledWith(plugin);
    act(() => groupToggle.click());
    expect(host.querySelector("[data-plugin-item]")).toBeNull();
    expect(groupToggle.lastElementChild?.textContent).toBe("2");
    act(() => groupToggle.click());
    expect(host.querySelectorAll("[data-plugin-item]")).toHaveLength(2);

    render([]);
    expect(installedGroup().children).toHaveLength(1);
    expect(host.querySelector('[role="status"]')).toBeNull();
  } finally {
    act(() => root.unmount());
    host.remove();
  }
});
