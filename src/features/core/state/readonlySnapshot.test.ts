import {
  installGraphProjectionFixture,
  makeEditorProjectionFixture,
} from "@/tests/helpers/editorProjectionFixtures";
import { afterEach, expect, it } from "vitest";
import { getGraphSnapshot } from "@/features/core/graph/read";
import { getResourceSnapshot } from "@/features/core/resource/read";
import { useGraphProjectionStore } from "@/features/core/dataStore/graphProjectionStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { markResourceLoaded } from "@/features/core/resource/documentStateActions";
import { buildFileResourceMeta, resourceKey } from "@/features/core/resource/resourceTypes";

afterEach(() => {
  useGraphProjectionStore.getState().clear();
  useGraphMetaStore.getState().clear();
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
});

it("keeps unrelated graph and resource snapshots stable and freezes published store records without copying", () => {
  const first = makeEditorProjectionFixture({ graphPath: "events/A" });
  const second = makeEditorProjectionFixture({ graphPath: "events/B" });
  installGraphProjectionFixture("events/A", first.projection);
  const graphBefore = getGraphSnapshot();
  installGraphProjectionFixture("events/B", second.projection);
  expect(getGraphSnapshot().graphEntities["events/A"]).toBe(graphBefore.graphEntities["events/A"]);
  expect(graphBefore.graphEntities["events/A"]).toBe(
    useGraphProjectionStore.getState().graphEntities["events/A"],
  );
  expect(Object.isFrozen(graphBefore.graphEntities["events/A"].nodes)).toBe(true);
  const graphs = getGraphSnapshot().graphEntities;
  useGraphMetaStore.setState({ graphs: { "events/B": { type: "event_graph" } } });
  expect(getGraphSnapshot().graphEntities).toBe(graphs);

  useResourceStore.getState().setSnapshot({
    resources: [
      buildFileResourceMeta("event_graph", "events/A", "A"),
      buildFileResourceMeta("event_graph", "events/B", "B"),
    ],
  });
  const refA = { id: "events/A", kind: "event_graph" } as const;
  markResourceLoaded(refA);
  const resourceBefore = getResourceSnapshot();
  markResourceLoaded({ id: "events/B", kind: "event_graph" });
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
