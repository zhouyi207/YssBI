import { afterEach, expect, it } from "vitest";
import { getGraphSnapshot } from "@/features/core/graph/read";
import { getResourceSnapshot } from "@/features/core/resource/read";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { markResourceLoaded } from "@/features/core/resource/documentStateActions";
import { buildGraphResourceMeta, resourceKey } from "@/features/core/resource/resourceTypes";
import { makeEditorProjectionFixture } from "@/tests/helpers/editorProjectionFixtures";

afterEach(() => {
  useGraphProjectionStore.setState({ graphEntities: {} });
  useGraphMetaStore.getState().clear();
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
});

it("keeps unrelated graph and resource snapshots stable and freezes published store records without copying", () => {
  const first = makeEditorProjectionFixture({ graphPath: "events/A" });
  const second = makeEditorProjectionFixture({ graphPath: "events/B" });
  const projections = useGraphProjectionStore.getState();
  projections.replaceProjection("events/A", first.projection);
  const graphBefore = getGraphSnapshot();
  projections.replaceProjection("events/B", second.projection);
  expect(getGraphSnapshot().graphEntities["events/A"]).toBe(graphBefore.graphEntities["events/A"]);
  expect(graphBefore.graphEntities["events/A"]).toBe(
    useGraphProjectionStore.getState().graphEntities["events/A"],
  );
  expect(Object.isFrozen(graphBefore.graphEntities["events/A"].nodes)).toBe(true);
  const graphs = getGraphSnapshot().graphEntities;
  useGraphMetaStore.getState().addGraph({ path: "events/B", name: "B", type: "event" });
  expect(getGraphSnapshot().graphEntities).toBe(graphs);

  useResourceStore
    .getState()
    .setResources([
      buildGraphResourceMeta("event", "events/A", "A"),
      buildGraphResourceMeta("event", "events/B", "B"),
    ]);
  const refA = { id: "events/A", kind: "event" } as const;
  markResourceLoaded(refA);
  const resourceBefore = getResourceSnapshot();
  markResourceLoaded({ id: "events/B", kind: "event" });
  expect(getResourceSnapshot().documents[resourceKey(refA)]).toBe(
    resourceBefore.documents[resourceKey(refA)],
  );
  expect(getResourceSnapshot().resources[resourceKey(refA)]).toBe(
    resourceBefore.resources[resourceKey(refA)],
  );
  expect(getResourceSnapshot().documents[resourceKey(refA)]).toBe(
    useDocumentStateStore.getState().documents[resourceKey(refA)],
  );
  const resourceAfter = getResourceSnapshot();
  useDocumentStateStore.setState({});
  expect(getResourceSnapshot()).toBe(resourceAfter);
});
