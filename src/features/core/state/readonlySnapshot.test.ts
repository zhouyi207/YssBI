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
import { freezePublishedValue, isPublishedValue } from "@/shared/types/deepReadonly";

afterEach(() => {
  useGraphProjectionStore.getState().clear();
  useGraphMetaStore.getState().clear();
  useResourceStore.getState().clear();
  useDocumentStateStore.getState().clear();
});

it("deep-freezes owned records and collections without treating a shallow freeze as publication", () => {
  const inherited = { external: { value: 1 } };
  const record = Object.assign(Object.create(inherited), { leaf: { value: 2 }, empty: [] });
  const indexed = Object.assign(Object.create(null), { node: record });
  const key = { id: "key" };
  const mapped = { value: 3 };
  const members = [{ value: 4 }];
  const root = { indexed, list: [record], map: new Map([[key, mapped]]), set: new Set(members) };
  record.parent = root;
  Object.freeze(root);
  expect(isPublishedValue(root)).toBe(false);

  expect(freezePublishedValue(root)).toBe(root);
  expect(isPublishedValue(root)).toBe(true);
  for (const value of [
    indexed,
    record,
    record.leaf,
    record.empty,
    root.list,
    key,
    mapped,
    ...members,
  ])
    expect(Object.isFrozen(value)).toBe(true);
  expect(() => {
    record.leaf.value = 9;
  }).toThrow(TypeError);
  expect(Object.isFrozen(inherited.external)).toBe(false);
  expect(freezePublishedValue(root)).toBe(root);

  const leaf = { value: 5 };
  freezePublishedValue(leaf);
  expect(isPublishedValue(leaf)).toBe(true);
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
