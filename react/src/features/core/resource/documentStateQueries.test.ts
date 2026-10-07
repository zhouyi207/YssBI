import { beforeEach, describe, expect, it } from "vitest";
import {
  clearResourceDocumentState,
  markResourceDirty,
  markResourceLoaded,
  useResourceStore,
  buildFileResourceMeta,
  isGraphResourceDirty,
  resourceKey,
} from "@/features/core/resource";

describe("document state queries", () => {
  beforeEach(() => {
    useResourceStore.getState().clear();
  });

  it("tracks dirty via DocumentState as single source of truth", () => {
    const meta = buildFileResourceMeta("event_graph", "events/A.yssbi-event", "A");
    useResourceStore.getState().setSnapshot({ resources: [meta] });
    markResourceLoaded({ id: meta.id, kind: "event_graph" });

    expect(isGraphResourceDirty(meta.id)).toBe(false);
    markResourceDirty({ id: meta.id, kind: "event_graph" }, true);
    expect(isGraphResourceDirty(meta.id)).toBe(true);
    expect(useResourceStore.getState().resources[resourceKey(meta)]?.hasDirtyDocument).toBe(true);
  });

  it("clears document state while retaining resource meta", () => {
    const meta = buildFileResourceMeta("event_graph", "events/A.yssbi-event", "A");
    useResourceStore.getState().setSnapshot({ resources: [meta] });
    markResourceLoaded({ id: meta.id, kind: "event_graph" });

    clearResourceDocumentState({ id: meta.id, kind: "event_graph" });

    expect(useResourceStore.getState().resources[resourceKey(meta)]).toMatchObject({
      loaded: false,
      exists: true,
    });
    expect(
      useResourceStore.getState().documents[resourceKey({ id: meta.id, kind: "event_graph" })],
    ).toBeUndefined();
  });
});
