import type { TFunction } from "i18next";
import { createStore, type StoreApi } from "zustand/vanilla";
import { shallow } from "zustand/shallow";
import type { InstalledPlugin, PluginManifest, PluginView } from "@/shared/types/plugins/generated";
import { freezePublishedValue, type DeepReadonly } from "@/shared/types/deepReadonly";
import { pluginService } from "@/services/plugins/pluginService";
import { installLocalPlugin, uninstallPlugin } from "./pluginActions";
import { openPluginWorkbenchView, syncPluginWorkbenchViews } from "@/modules/workbench/public";

export type PluginSnapshot = DeepReadonly<InstalledPlugin>;
export interface PluginRegistrySnapshot {
  readonly plugins: readonly PluginSnapshot[];
  readonly byId: ReadonlyMap<string, PluginSnapshot>;
  readonly loading: boolean;
  readonly busy: boolean;
  readonly error: "plugins.loadFailed" | "plugins.operationFailed" | null;
}

function projectManifest(
  previous: DeepReadonly<PluginManifest>,
  next: DeepReadonly<PluginManifest>,
): DeepReadonly<PluginManifest> {
  const commands = next.contributes.commands.map((command, index) =>
    shallow(previous.contributes.commands[index], command)
      ? previous.contributes.commands[index]
      : command,
  );
  const taskTypes = next.contributes.taskTypes.map((taskType, index) =>
    shallow(previous.contributes.taskTypes[index], taskType)
      ? previous.contributes.taskTypes[index]
      : taskType,
  );
  const views = next.contributes.views.map((view, index) =>
    shallow(previous.contributes.views[index], view) ? previous.contributes.views[index] : view,
  );
  const contributes = {
    ...next.contributes,
    commands: shallow(previous.contributes.commands, commands)
      ? previous.contributes.commands
      : commands,
    taskTypes: shallow(previous.contributes.taskTypes, taskTypes)
      ? previous.contributes.taskTypes
      : taskTypes,
    views: shallow(previous.contributes.views, views) ? previous.contributes.views : views,
  };
  const protocol = {
    ...next.protocol,
    requiredFeatures: shallow(previous.protocol.requiredFeatures, next.protocol.requiredFeatures)
      ? previous.protocol.requiredFeatures
      : next.protocol.requiredFeatures,
  };
  const manifest = {
    ...next,
    contributes: shallow(previous.contributes, contributes) ? previous.contributes : contributes,
    protocol: shallow(previous.protocol, protocol) ? previous.protocol : protocol,
    permissions: shallow(previous.permissions, next.permissions)
      ? previous.permissions
      : next.permissions,
    uiMethods: shallow(previous.uiMethods, next.uiMethods) ? previous.uiMethods : next.uiMethods,
    resourceBudget: shallow(previous.resourceBudget, next.resourceBudget)
      ? previous.resourceBudget
      : next.resourceBudget,
  };
  if ("cacheDirectories" in manifest && shallow(previous.cacheDirectories, next.cacheDirectories))
    manifest.cacheDirectories = previous.cacheDirectories;
  return shallow(previous, manifest) ? previous : manifest;
}

function projectPlugin(previous: PluginSnapshot | undefined, next: PluginSnapshot): PluginSnapshot {
  if (!previous || previous === next) return next;
  const plugin = {
    ...next,
    // Rust hashes the complete signed manifest into packageDigest. Enable/disable can
    // advance the installation generation without changing this immutable package.
    manifest:
      previous.packageDigest === next.packageDigest
        ? previous.manifest
        : projectManifest(previous.manifest, next.manifest),
  };
  if ("grantedBudget" in plugin && shallow(previous.grantedBudget, next.grantedBudget))
    plugin.grantedBudget = previous.grantedBudget;
  return shallow(previous, plugin) ? previous : plugin;
}

/** A mounted workbench owns one registry projection; Rust remains the installation authority. */
export function createPluginRegistry() {
  const store = createStore<PluginRegistrySnapshot>(() => ({
    plugins: [],
    byId: new Map(),
    loading: true,
    busy: false,
    error: null,
  }));
  let lifecycle: object | null = null;
  let refreshEpoch = 0;
  let running: object | null = null;

  const publish = async (next: readonly PluginSnapshot[], isCurrent: () => boolean) => {
    if (!isCurrent()) return;
    const previous = store.getState();
    const shared = next.map((plugin) =>
      projectPlugin(previous.byId.get(plugin.manifest.id), plugin),
    );
    const plugins = shallow(previous.plugins, shared)
      ? previous.plugins
      : freezePublishedValue(shared);
    const candidate: PluginRegistrySnapshot = {
      ...previous,
      plugins,
      byId:
        plugins === previous.plugins
          ? previous.byId
          : freezePublishedValue(new Map(plugins.map((plugin) => [plugin.manifest.id, plugin]))),
      error: null,
      loading: false,
    };
    if (!shallow(previous, candidate)) store.setState(candidate);
    await syncPluginWorkbenchViews(
      plugins.map((plugin) => plugin.manifest.id),
      plugins
        .filter((plugin) => plugin.enabled)
        .flatMap((plugin) =>
          plugin.manifest.contributes.views
            .filter((view) => view.location === "sidebar")
            .map((view) => ({
              pluginId: plugin.manifest.id,
              viewId: view.id,
              title: view.title,
              location: view.location,
            })),
        ),
      () => isCurrent() && store.getState().plugins === plugins,
    );
  };

  const refresh = async () => {
    const owner = lifecycle;
    if (!owner) return;
    const epoch = ++refreshEpoch;
    const isCurrent = () => lifecycle === owner && epoch === refreshEpoch;
    try {
      const next = await pluginService.list();
      await publish(next, isCurrent);
    } catch {
      if (isCurrent()) store.setState({ error: "plugins.loadFailed", loading: false });
    }
  };

  const run = async (action: (isCurrent: () => boolean) => Promise<unknown>) => {
    const owner = lifecycle;
    if (!owner || running) return;
    const operation = {};
    running = operation;
    const isCurrent = () => lifecycle === owner && running === operation;
    store.setState({ busy: true, error: null });
    try {
      await action(isCurrent);
      if (isCurrent()) await refresh();
    } catch {
      if (isCurrent()) store.setState({ error: "plugins.operationFailed" });
    } finally {
      if (isCurrent()) {
        running = null;
        store.setState({ busy: false });
      }
    }
  };

  const actions = {
    refresh,
    install: (t: TFunction) => run((isCurrent) => installLocalPlugin(t, isCurrent)),
    uninstall: (plugin: PluginSnapshot, t: TFunction) =>
      run(async (isCurrent) => {
        const targetIsCurrent = () =>
          isCurrent() &&
          store.getState().byId.get(plugin.manifest.id)?.installationGeneration ===
            plugin.installationGeneration;
        if (
          !(await uninstallPlugin(plugin.manifest.id, plugin.manifest.name, t, targetIsCurrent)) ||
          !isCurrent()
        )
          return;
        // The committed removal remains authoritative when a later registry query fails.
        refreshEpoch += 1;
        await publish(
          store
            .getState()
            .plugins.filter(
              (entry) =>
                entry.manifest.id !== plugin.manifest.id ||
                entry.installationGeneration !== plugin.installationGeneration,
            ),
          isCurrent,
        );
      }),
    setEnabled: (plugin: PluginSnapshot) =>
      run(async () => {
        const current = store.getState().byId.get(plugin.manifest.id);
        if (current?.installationGeneration === plugin.installationGeneration)
          await pluginService.setEnabled(current.manifest.id, !current.enabled);
      }),
    open: (pluginId: string, view: DeepReadonly<PluginView>) => {
      const owner = lifecycle;
      const plugin = store.getState().byId.get(pluginId);
      const contribution = plugin?.manifest.contributes.views.find((entry) => entry.id === view.id);
      if (!owner || !plugin?.enabled || !contribution) return;
      void openPluginWorkbenchView(
        {
          pluginId,
          viewId: contribution.id,
          title: contribution.title,
          location: contribution.location,
        },
        true,
        () => {
          const current = store.getState().byId.get(pluginId);
          return (
            lifecycle === owner &&
            !!current?.enabled &&
            current.installationGeneration === plugin.installationGeneration
          );
        },
      );
    },
  };

  return {
    read: store as Pick<
      StoreApi<PluginRegistrySnapshot>,
      "getState" | "getInitialState" | "subscribe"
    >,
    actions,
    start: () => {
      lifecycle = {};
      store.setState({ loading: true });
      return refresh();
    },
    stop: () => {
      lifecycle = null;
      refreshEpoch += 1;
      running = null;
      store.setState({ busy: false, loading: false });
    },
  };
}
