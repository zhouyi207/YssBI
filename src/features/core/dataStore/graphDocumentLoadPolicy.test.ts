import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { describe, expect, it, beforeEach } from "vitest";
import { useGraphProjectionStore } from "./graphProjectionStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import {
  buildGraphResourceMeta,
  markResourceLoaded,
  resourceKey,
  useResourceStore,
} from "@/features/core/resource";
import { isGraphCachedInMemory } from "./graphDocumentLoadPolicy";

describe("graphDocumentLoadPolicy", () => {
  const graphPath = "opaque graph resource";
  const docKey = resourceKey({ id: graphPath, kind: "event" });

  beforeEach(() => {
    useGraphProjectionStore.getState().clear();
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().setResources([buildGraphResourceMeta("event", graphPath, "Main")]);
  });

  it("returns false when graph is not in memory", () => {
    expect(isGraphCachedInMemory("events/Missing.yssbi-event")).toBe(false);
  });

  it("returns false when no authoritative resource metadata exists", () => {
    const fixture = makeEditorProjectionFixture({ graphPath: "evt-1" });
    installGraphProjectionFixture("evt-1", fixture.projection);
    expect(isGraphCachedInMemory("evt-1")).toBe(false);
  });

  it("returns false when a bucket exists for an unloaded graph resource", () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    installGraphProjectionFixture(graphPath, fixture.projection);

    expect(isGraphCachedInMemory(graphPath)).toBe(false);
  });

  it("returns true when graph is cached and document is clean", () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    installGraphProjectionFixture(graphPath, fixture.projection);
    markResourceLoaded({ id: graphPath, kind: "event" });

    expect(isGraphCachedInMemory(graphPath)).toBe(true);
  });

  it("returns false when graph is stale", () => {
    const fixture = makeEditorProjectionFixture({ graphPath });
    installGraphProjectionFixture(graphPath, fixture.projection);
    markResourceLoaded({ id: graphPath, kind: "event" });
    useDocumentStateStore.getState().patchDocument(docKey, { stale: true });

    expect(isGraphCachedInMemory(graphPath)).toBe(false);
  });
});
