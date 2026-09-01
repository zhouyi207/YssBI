import { beforeEach, describe, expect, it } from "vitest";
import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
  useDocumentStateStore,
  useResourceStore,
  buildGraphResourceMeta,
  isGraphResourceDirty,
  resourceKey,
} from "@/features/core/resource";
import { collectDirtyEditorPanels } from "@/features/core/layout/editorPanelDirty";

describe("document state queries", () => {
  beforeEach(() => {
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
  });

  it("tracks dirty via DocumentState as single source of truth", () => {
    const meta = buildGraphResourceMeta("event", "events/A.yssbi-event", "A");
    useResourceStore.getState().upsertResource(meta);
    markResourceLoaded({ id: meta.id, kind: "event" });

    expect(isGraphResourceDirty(meta.id, "event")).toBe(false);
    markResourceDirty({ id: meta.id, kind: "event" }, true);
    expect(isGraphResourceDirty(meta.id, "event")).toBe(true);
    expect(useResourceStore.getState().resources[resourceKey(meta)]?.hasDirtyDocument).toBe(true);
  });

  it("clears document state while retaining resource meta", () => {
    const meta = buildGraphResourceMeta("event", "events/A.yssbi-event", "A");
    useResourceStore.getState().upsertResource(meta);
    markResourceLoaded({ id: meta.id, kind: "event" });

    clearResourceDocumentState({ id: meta.id, kind: "event" });

    expect(useResourceStore.getState().resources[resourceKey(meta)]).toMatchObject({
      loaded: false,
      exists: true,
    });
    expect(
      useDocumentStateStore.getState().documents[resourceKey({ id: meta.id, kind: "event" })],
    ).toBeUndefined();
  });
});

describe("collectDirtyEditorPanels", () => {
  beforeEach(() => {
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
  });

  it("reads dirty from DocumentState instead of panel metadata", () => {
    const path = "events/A.yssbi-event";
    useResourceStore.getState().upsertResource(buildGraphResourceMeta("event", path, "A"));
    markResourceDirty({ id: path, kind: "event" }, true);

    expect(collectDirtyEditorPanels()).toEqual([]);
  });
});
