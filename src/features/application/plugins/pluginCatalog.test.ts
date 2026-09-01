import { describe, expect, it } from "vitest";

import {
  BUILT_IN_PLUGIN_MANIFESTS,
  getInstalledPluginManifests,
  getPluginManifest,
} from "./pluginCatalog";

describe("plugin catalog", () => {
  it("publishes Julia as an installable built-in plugin", () => {
    const julia = getPluginManifest("julia");

    expect(julia).toMatchObject({
      id: "julia",
      titleKey: "plugins.julia.title",
      icon: "julia",
    });
    expect(BUILT_IN_PLUGIN_MANIFESTS).toContain(julia);
  });

  it("projects only known installed plugin manifests in catalog order", () => {
    expect(getInstalledPluginManifests(["unknown", "julia"])).toEqual([getPluginManifest("julia")]);
  });
});
