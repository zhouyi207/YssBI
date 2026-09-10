import { activityPanelFixture } from "@/tests/helpers/activityPanelFixture";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { useSidebarStore } from "./sidebarStore";

describe("Activity category expansion", () => {
  beforeEach(() => {
    useSidebarStore.setState({ expandedCategories: {} });
  });
  afterEach(() => {
    vi.unstubAllGlobals();
  });
  it("keeps expansion independent across panels and leaves defaults to the backend", () => {
    const store = useSidebarStore.getState();
    store.setCategoryExpanded("project", "shared-id", true);
    store.setCategoryExpanded("nodes", "shared-id", false);
    expect(useSidebarStore.getState().expandedCategories).toEqual({
      project: { "shared-id": true },
      nodes: { "shared-id": false },
    });
  });
  it("loads only boolean UI preferences and persists through the same key", async () => {
    const persisted = new Map<string, string>([
      [
        "yssbi-activity-panel-expansion",
        JSON.stringify({
          project: { events: true, bad: "true" },
          plugins: { installed: false },
          unknown: { x: true },
        }),
      ],
    ]);
    vi.stubGlobal("localStorage", {
      getItem: (key: string) => persisted.get(key) ?? null,
      setItem: (key: string, value: string) => persisted.set(key, value),
    });
    vi.resetModules();
    const fresh = await import("./sidebarStore");
    expect(fresh.useSidebarStore.getState().expandedCategories).toEqual({
      project: { events: true },
      plugins: { installed: false },
    });
    const binding = fresh.useSidebarStore
      .getState()
      .bindPanel({ panelId: "plugins", projectInstanceId: null, locale: "en-US", epoch: 0 });
    fresh.useSidebarStore
      .getState()
      .publishPanels([
        { binding, snapshot: { cursor: "c1", document: activityPanelFixture("plugins", []) } },
      ]);
    fresh.useSidebarStore.getState().setCategoryExpanded("plugins", "installed", true);
    expect(JSON.parse(persisted.get("yssbi-activity-panel-expansion")!)).toEqual({
      project: { events: true },
      plugins: { installed: true },
    });
  });
});
