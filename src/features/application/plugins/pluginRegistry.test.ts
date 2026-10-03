import { beforeEach, expect, it, vi } from "vitest";
import type { InstalledPlugin } from "@/shared/types/plugins/generated";
import { createPluginRegistry } from "./pluginRegistry";

const mocks = vi.hoisted(() => ({ list: vi.fn(), sync: vi.fn() }));
vi.mock("@/services/plugins/pluginService", () => ({ pluginService: { list: mocks.list } }));
vi.mock("./pluginActions", () => ({ installLocalPlugin: vi.fn(), uninstallPlugin: vi.fn() }));
vi.mock("@/modules/workbench/public", () => ({
  syncPluginWorkbenchViews: mocks.sync,
  openPluginWorkbenchView: vi.fn(),
}));

function installed(id = "example.plugin"): InstalledPlugin {
  const budget = {
    frameBytes: 1024,
    pendingRequests: 1,
    activeTasks: 1,
    queuedBytes: 1024,
    snapshotBytes: 1024,
    privateStorageBytes: 1024,
    views: 2,
  };
  return {
    enabled: true,
    installationGeneration: "1",
    packageDigest: `digest-${id}`,
    signerKey: "signer",
    processState: "stopped",
    grantedBudget: { ...budget },
    manifest: {
      id,
      name: id,
      publisher: "example",
      version: "1.0.0",
      description: "",
      schemaVersion: 1,
      hostApi: "^2",
      target: "test",
      executable: "bin/app",
      execution: "trustedNative",
      protocol: { major: 2, minMinor: 0, maxMinor: 0, requiredFeatures: [] },
      permissions: ["project.read"],
      uiMethods: ["views.get_state"],
      cacheDirectories: ["cache"],
      resourceBudget: budget,
      contributes: {
        commands: [{ id: "refresh", title: "Refresh" }],
        taskTypes: [{ id: "analyze", producesArtifacts: true }],
        views: [
          {
            id: "runtime",
            title: "Runtime",
            entry: "index.html",
            scope: "application",
            location: "sidebar",
          },
          {
            id: "analysis",
            title: "Analysis",
            entry: "index.html",
            scope: "project",
            location: "editor",
          },
        ],
      },
    },
  };
}

beforeEach(() => vi.resetAllMocks());

it("shares immutable package content while accepting runtime, grant and installation changes", async () => {
  const initial = [installed(), installed("example.other")];
  mocks.list.mockResolvedValue(structuredClone(initial));
  const registry = createPluginRegistry();
  await registry.start();
  const before = registry.read.getState();
  const published = vi.fn();
  const unsubscribe = registry.read.subscribe(published);
  try {
    mocks.list.mockResolvedValue(structuredClone(initial));
    await registry.actions.refresh();
    expect(registry.read.getState()).toBe(before);
    expect(published).not.toHaveBeenCalled();

    const running = structuredClone(initial);
    running[0].processState = "running";
    running[0].grantedBudget!.views = 1;
    mocks.list.mockResolvedValue(running);
    await registry.actions.refresh();
    const active = registry.read.getState();
    expect(active.plugins[0]).toEqual(running[0]);
    expect(active.plugins[0].manifest).toBe(before.plugins[0].manifest);
    expect(active.plugins[1]).toBe(before.plugins[1]);
    expect(active.byId.get("example.plugin")).toBe(active.plugins[0]);

    const disabled = structuredClone(running);
    disabled[0].enabled = false;
    disabled[0].installationGeneration = "2";
    disabled[0].processState = "stopped";
    mocks.list.mockResolvedValue(disabled);
    await registry.actions.refresh();
    const after = registry.read.getState();
    expect(after.plugins[0]).toEqual(disabled[0]);
    expect(after.plugins[0].manifest).toBe(before.plugins[0].manifest);
    expect(after.plugins[0].grantedBudget).toBe(active.plugins[0].grantedBudget);
    expect(before.plugins).toEqual(initial);
    expect(published).toHaveBeenCalledTimes(2);
    expect(mocks.sync.mock.lastCall?.[1]).toEqual([
      { pluginId: "example.other", viewId: "runtime", title: "Runtime", location: "sidebar" },
    ]);
  } finally {
    unsubscribe();
    registry.stop();
  }
});

it("installs changed package fields and removals while preserving unchanged manifest branches", async () => {
  const initial = installed();
  mocks.list.mockResolvedValue(structuredClone([initial]));
  const registry = createPluginRegistry();
  await registry.start();
  const before = registry.read.getState().plugins[0];
  try {
    const replacement = structuredClone(initial);
    replacement.packageDigest = "replacement-digest";
    replacement.installationGeneration = "2";
    replacement.signerKey = "replacement-signer";
    replacement.manifest.version = "2.0.0";
    replacement.manifest.description = "Updated package";
    replacement.manifest.protocol.requiredFeatures = ["progress"];
    replacement.manifest.contributes.views[0].title = "New runtime";
    replacement.manifest.contributes.commands = [];
    delete replacement.manifest.cacheDirectories;
    delete replacement.grantedBudget;
    mocks.list.mockResolvedValue([replacement]);
    await registry.actions.refresh();
    const after = registry.read.getState();
    expect(after.plugins[0]).toEqual(replacement);
    const manifest = after.plugins[0].manifest;
    expect(manifest.permissions).toBe(before.manifest.permissions);
    expect(manifest.uiMethods).toBe(before.manifest.uiMethods);
    expect(manifest.resourceBudget).toBe(before.manifest.resourceBudget);
    expect(manifest.contributes.taskTypes).toBe(before.manifest.contributes.taskTypes);
    expect(manifest.contributes.views[1]).toBe(before.manifest.contributes.views[1]);
    expect(manifest.contributes.views[0]).not.toBe(before.manifest.contributes.views[0]);
    expect(manifest.protocol).not.toBe(before.manifest.protocol);
    expect(before).toEqual(initial);
    expect(mocks.sync.mock.lastCall?.[1][0].title).toBe("New runtime");

    mocks.list.mockResolvedValue(structuredClone([replacement]));
    await registry.actions.refresh();
    expect(registry.read.getState()).toBe(after);
  } finally {
    registry.stop();
  }
});
