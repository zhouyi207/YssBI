import { ChartService } from "@/services/chart/chartService";
import { useChartDocumentStore } from "@/features/core/chart/chartDocumentStore";
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
    const store = useChartDocumentStore.getState();
    const cached = store.documents[chartPath];
    if (cached) return cached;
    const key = JSON.stringify([identity.projectInstanceId, identity.epoch, chartPath]);
    const pending = loads.get(key);
    if (pending?.isCurrent()) return await pending.promise;

    const resourceId = resourceKey({ id: chartPath, kind: "chart" });
    const resource = useResourceStore.getState().resources[resourceId];
    if (resource?.exists === false) return null;
    const ownsRead = store.beginRead(chartPath);
    const isCurrent = () => {
      const current = useResourceStore.getState().resources[resourceId];
      return (
        isCurrentProjectIdentity(identity) &&
        ownsRead() &&
        current?.revision === resource?.revision &&
        current?.exists === resource?.exists
      );
    };
    const promise = ChartService.loadChart(identity.projectInstanceId, chartPath)
      .then((document) => {
        if (!isCurrentProjectIdentity(identity)) return null;
        const current = useChartDocumentStore.getState().documents[chartPath];
        if (current) return current;
        if (!isCurrent()) return null;
        useChartDocumentStore.getState().upsertDocument(chartPath, document);
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
