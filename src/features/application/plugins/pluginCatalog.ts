export type PluginIcon = "julia";

export interface PluginManifest {
  readonly id: string;
  readonly titleKey: string;
  readonly descriptionKey: string;
  readonly icon: PluginIcon;
  readonly order: number;
}

export const JULIA_PLUGIN_ID = "julia";

export const BUILT_IN_PLUGIN_MANIFESTS: readonly PluginManifest[] = [
  {
    id: JULIA_PLUGIN_ID,
    titleKey: "plugins.julia.title",
    descriptionKey: "plugins.julia.description",
    icon: "julia",
    order: 10,
  },
];

export function getPluginManifest(pluginId: string): PluginManifest | undefined {
  return BUILT_IN_PLUGIN_MANIFESTS.find((manifest) => manifest.id === pluginId);
}

export function getInstalledPluginManifests(
  installedPluginIds: readonly string[],
): PluginManifest[] {
  const installed = new Set(installedPluginIds);

  return BUILT_IN_PLUGIN_MANIFESTS.filter((manifest) => installed.has(manifest.id)).sort(
    (left, right) => left.order - right.order,
  );
}
