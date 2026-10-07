import { ChartService } from "@/services/chart/chartService";
import {
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { resourceKey, useResourceStore } from "@/features/core/resource";
import type { ChartDocument } from "@/shared/types/domain/chart";

const loads = new Map<string, { isCurrent(): boolean; promise: Promise<ChartDocument | null> }>();

/** Share initial reads without replacing a document installed or edited while awaiting IPC. */
export async function loadChartDocumentForView(chartPath: string): Promise<ChartDocument | null> {
  try {
    const identity = captureProjectIdentity();
    const store = useResourceStore.getState();
    const cached = store.chartDocuments[chartPath];
    if (cached) return cached;
    const key = JSON.stringify([
      identity.projectInstanceId,
      identity.epoch,
      store.indexRevision,
      chartPath,
    ]);
    const pending = loads.get(key);
    if (pending?.isCurrent()) return await pending.promise;

    const resourceId = resourceKey({ id: chartPath, kind: "chart" });
    const resource = store.resources[resourceId];
    if (!resource?.exists) return null;
    const ownsRead = store.beginChartRead(chartPath);
    const isCurrent = () => {
      const current = useResourceStore.getState().resources[resourceId];
      return (
        isCurrentProjectIdentity(identity) &&
        ownsRead() &&
        current?.revision === resource?.revision &&
        current?.exists === resource?.exists
      );
    };
    const promise = ChartService.loadChart(
      identity.projectInstanceId,
      chartPath,
      store.indexRevision,
    )
      .then((document) => {
        if (!isCurrentProjectIdentity(identity)) return null;
        const current = useResourceStore.getState().chartDocuments[chartPath];
        if (current) return current;
        if (!isCurrent()) return null;
        useResourceStore.getState().upsertChartDocument(chartPath, document);
        return document;
      })
      .catch(() => null)
      .finally(() => {
        if (loads.get(key)?.promise === promise) loads.delete(key);
      });
    loads.set(key, { isCurrent, promise });
    return await promise;
  } catch {
    return null;
  }
}
