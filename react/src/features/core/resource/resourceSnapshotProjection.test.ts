import { beforeEach, describe, expect, it } from "vitest";
import {
  useResourceStore,
  buildFileResourceMeta,
  type ProjectResourceMeta,
} from "@/features/core/resource";
import { prepareResourceProjectionSnapshot } from "./resourceSnapshotProjection";
import { resourceKey } from "./resourceTypes";

function graphResource(
  id: string,
  kind: "event_graph" | "function_graph",
  name: string,
): ProjectResourceMeta {
  return buildFileResourceMeta(kind, id, name);
}

describe("resource projection snapshot preparation", () => {
  beforeEach(() => {
    useResourceStore.getState().clear();
  });

  it("marks loaded clean resources stale when snapshot metadata changes", () => {
    const previous = graphResource("g1", "event_graph", "Old Name");
    previous.loaded = true;
    useResourceStore.getState().upsertDocument({
      resourceKey: resourceKey(previous),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
    });

    const incoming = [graphResource("g1", "event_graph", "New Name")];
    const { resources, documentPatches } = prepareResourceProjectionSnapshot(incoming, {
      [resourceKey(previous)]: previous,
    });

    expect(resources[0]).toMatchObject({
      loaded: true,
      hasStaleDocument: true,
      hasConflictDocument: false,
    });
    expect(documentPatches).toEqual([
      { key: resourceKey(previous), patch: { stale: true, conflict: false, missing: false } },
    ]);
  });

  it("retains missing loaded resources absent from the snapshot", () => {
    const previous = graphResource("g1", "event_graph", "Removed");
    previous.loaded = true;
    useResourceStore.getState().upsertDocument({
      resourceKey: resourceKey(previous),
      loaded: true,
      dirty: false,
      stale: false,
      missing: false,
      conflict: false,
    });

    const { resources, documentPatches } = prepareResourceProjectionSnapshot([], {
      [resourceKey(previous)]: previous,
    });

    expect(resources).toHaveLength(1);
    expect(resources[0]).toMatchObject({
      id: "g1",
      exists: false,
      loaded: true,
    });
    expect(documentPatches).toEqual([
      { key: resourceKey(previous), patch: { missing: true, stale: false, conflict: false } },
    ]);
  });
});
