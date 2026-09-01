import { beforeEach, describe, expect, it } from "vitest";

import { useGraphDataStore } from "@/features/core/dataStore/graphDataStore";
import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { useDocumentStateStore } from "@/features/core/resource/documentStateStore";
import { useResourceStore } from "@/features/core/resource/resourceStore";
import { useHistoryStore } from "@/features/core/history/historyStore";
import { EMPTY_HISTORY_STATE } from "@/features/core/history/historyStore";
import { getGraphSnapshot } from "./read";
import {
  createGraphProjectionPublication,
  optimisticOperationKey,
  type OptimisticOperationKey,
} from "./publication";
import { getResourceSnapshot } from "@/features/core/resource/read";
import { createResourceProjectionPublication } from "@/features/core/resource/publication";
import { getHistorySnapshot } from "@/features/core/history/read";
import { createHistoryProjectionPublication } from "@/features/core/history/publication";

const key = (overrides: Partial<OptimisticOperationKey> = {}): OptimisticOperationKey => ({
  projectInstanceId: "project-a",
  resourceKey: "events/main.yssbi-event",
  operationId: "operation-a",
  fromRevision: 4,
  ...overrides,
});

describe("staged graph/resource/history capabilities", () => {
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
    useGraphMetaStore.setState({ graphs: {} });
    useResourceStore.setState({ resources: {}, graphOrder: [] });
    useDocumentStateStore.setState({ documents: {} });
    useHistoryStore.setState(EMPTY_HISTORY_STATE);
  });

  it("isolates optimistic overlays by all four key fields and cleans only the exact key", () => {
    const publication = createGraphProjectionPublication();
    const current = key();
    const nextRevision = key({ operationId: "operation-b", fromRevision: 5 });
    const oldProject = key({ projectInstanceId: "project-old" });

    publication.beginOptimisticOverlay(current, { kind: "move", nodeIds: ["node-a"] });
    publication.beginOptimisticOverlay(nextRevision, { kind: "move", nodeIds: ["node-b"] });

    expect(optimisticOperationKey(current)).not.toBe(optimisticOperationKey(nextRevision));
    expect(publication.getOptimisticOverlay(current)).toEqual({
      kind: "move",
      nodeIds: ["node-a"],
    });
    expect(publication.settleOptimisticOverlay(nextRevision)).toBe("settled");
    expect(publication.getOptimisticOverlay(current)).toEqual({
      kind: "move",
      nodeIds: ["node-a"],
    });
    expect(publication.rejectOptimisticOverlay(oldProject)).toBe("missing");
    expect(publication.invalidateOptimisticOverlay(current)).toBe("invalidated");
    expect(publication.getOptimisticOverlay(current)).toBeUndefined();
    expect(publication.settleOptimisticOverlay(current)).toBe("missing");
  });

  it("publishes detached snapshots through the domain-specific publication capabilities", () => {
    const graphPublication = createGraphProjectionPublication();
    const resourcePublication = createResourceProjectionPublication();
    const historyPublication = createHistoryProjectionPublication();

    const graphInput = { graphEntities: {}, graphMeta: {} };
    graphPublication.replaceSnapshot(graphInput);
    expect(getGraphSnapshot()).not.toBe(graphInput);
    expect(getGraphSnapshot().graphEntities).not.toBe(graphInput.graphEntities);

    const resourceInput = { resources: {}, graphOrder: ["events/main"], documents: {} };
    resourcePublication.replaceSnapshot(resourceInput);
    expect(getResourceSnapshot()).not.toBe(resourceInput);
    expect(getResourceSnapshot().graphOrder).not.toBe(resourceInput.graphOrder);

    const historyInput = { canUndo: true, canRedo: false, pending: true };
    historyPublication.replaceSnapshot(historyInput);
    expect(getHistorySnapshot()).not.toBe(historyInput);
    expect(getHistorySnapshot()).toEqual(historyInput);
  });
});
