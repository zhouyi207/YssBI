import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import type { ChartDocument, ChartPreviewPayload } from "@/shared/types/domain/chart";
import { isChartDocument } from "./chartDocumentGuard";

export interface ChartPreviewIdentity {
  readonly projectInstanceId: string;
  readonly epoch: number;
}

export interface ChartPreviewRequest {
  readonly projectInstanceId: string;
  readonly databaseId: string;
  readonly chartPath: string;
  readonly requestGeneration: number;
  readonly document: DeepReadonly<ChartDocument>;
}

export interface ChartPreviewServicePort {
  query(
    projectInstanceId: string,
    databaseId: string,
    chartPath: string,
    document: ChartDocument,
  ): Promise<ChartPreviewPayload>;
}

export interface ChartPreviewPublication {
  publish(request: ChartPreviewRequest, value: DeepReadonly<ChartPreviewPayload>): void;
  publishFailure(request: ChartPreviewRequest, issue: ErrorReference): void;
  clearForProject?(projectInstanceId: string | null): void;
}

export type ChartPreviewOutcome =
  | {
      readonly status: "published";
      readonly value: DeepReadonly<ChartPreviewPayload>;
    }
  | { readonly status: "stale" }
  | { readonly status: "notReady" }
  | { readonly status: "failed" };

export interface ChartPreviewCoordinator {
  query(chartPath: string, document: DeepReadonly<ChartDocument>): Promise<ChartPreviewOutcome>;
  resetProject(): void;
  getCached(
    chartPath: string,
    document: DeepReadonly<ChartDocument>,
  ): DeepReadonly<ChartPreviewPayload> | undefined;
}

export interface ChartPreviewCoordinatorDependencies {
  readonly captureProjectIdentity: () => ChartPreviewIdentity | null;
  readonly service: ChartPreviewServicePort;
  readonly publication?: ChartPreviewPublication;
  readonly toErrorReference?: (error: unknown, fallbackCode: string) => ErrorReference;
}

const MAX_CACHE_ENTRIES = 32;

function fallbackIssue(): ErrorReference {
  return { code: "chart_preview_read_failed", incidentId: null };
}

function isErrorReference(value: unknown): value is ErrorReference {
  return (
    typeof value === "object" &&
    value !== null &&
    typeof (value as { code?: unknown }).code === "string" &&
    ((value as { incidentId?: unknown }).incidentId === null ||
      typeof (value as { incidentId?: unknown }).incidentId === "string")
  );
}

function cloneValue<T>(value: T): T {
  if (Array.isArray(value)) return value.map(cloneValue) as T;
  if (value === null || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
      key,
      cloneValue(nested),
    ]),
  ) as T;
}

function freezeValue<T>(value: T): T {
  if (Array.isArray(value)) return Object.freeze(value.map(freezeValue)) as T;
  if (value === null || typeof value !== "object") return value;
  return Object.freeze(
    Object.fromEntries(
      Object.entries(value as Record<string, unknown>).map(([key, nested]) => [
        key,
        freezeValue(nested),
      ]),
    ),
  ) as T;
}

function previewKey(
  projectInstanceId: string,
  chartPath: string,
  document: DeepReadonly<ChartDocument>,
): string {
  return JSON.stringify([
    projectInstanceId,
    chartPath,
    document.databaseId,
    document.chartType,
    document.encodings.x ?? null,
    document.encodings.y ?? null,
  ]);
}

function mutableDocument(document: DeepReadonly<ChartDocument>): ChartDocument {
  return {
    ...document,
    encodings: {
      x: document.encodings.x,
      y: document.encodings.y,
    },
  };
}

export function createChartPreviewCoordinator(
  dependencies: ChartPreviewCoordinatorDependencies,
): ChartPreviewCoordinator {
  const cache = new Map<string, DeepReadonly<ChartPreviewPayload>>();
  const inFlight = new Map<string, Promise<ChartPreviewOutcome>>();
  const latestGeneration = new Map<string, number>();
  let coordinatorEpoch = 0;
  let nextGeneration = 0;

  const captureIdentity = (): ChartPreviewIdentity | null => {
    try {
      return dependencies.captureProjectIdentity();
    } catch {
      return null;
    }
  };

  const issueFor = (error: unknown): ErrorReference => {
    try {
      const mapped = dependencies.toErrorReference?.(error, fallbackIssue().code);
      if (isErrorReference(mapped)) return mapped;
    } catch {
      // Error conversion is advisory; the closed fallback is authoritative.
    }
    return fallbackIssue();
  };

  const publishFailure = (request: ChartPreviewRequest, error: unknown): void => {
    try {
      dependencies.publication?.publishFailure(request, issueFor(error));
    } catch {
      // Failure presentation cannot change the closed query outcome.
    }
  };

  const current = (
    identity: ChartPreviewIdentity,
    ownerCoordinatorEpoch: number,
    chartPath: string,
    generation: number,
  ): boolean => {
    const latest = latestGeneration.get(chartPath);
    if (latest !== generation || coordinatorEpoch !== ownerCoordinatorEpoch) {
      return false;
    }
    const active = captureIdentity();
    return (
      active !== null &&
      active.projectInstanceId === identity.projectInstanceId &&
      active.epoch === identity.epoch
    );
  };

  const getCached = (
    chartPath: string,
    document: DeepReadonly<ChartDocument>,
  ): DeepReadonly<ChartPreviewPayload> | undefined => {
    const identity = captureIdentity();
    if (!identity || !isChartDocument(document)) return undefined;
    const key = previewKey(identity.projectInstanceId, chartPath, document);
    const value = cache.get(key);
    if (value !== undefined) {
      cache.delete(key);
      cache.set(key, value);
    }
    return value;
  };

  const query = async (
    chartPath: string,
    document: DeepReadonly<ChartDocument>,
  ): Promise<ChartPreviewOutcome> => {
    const identity = captureIdentity();
    if (!identity || !isChartDocument(document)) return { status: "notReady" };

    const key = previewKey(identity.projectInstanceId, chartPath, document);
    const existing = inFlight.get(key);
    if (existing) return existing;

    const generation = ++nextGeneration;
    const ownerCoordinatorEpoch = coordinatorEpoch;
    latestGeneration.set(chartPath, generation);
    const request: ChartPreviewRequest = {
      projectInstanceId: identity.projectInstanceId,
      databaseId: document.databaseId,
      chartPath,
      requestGeneration: generation,
      document: freezeValue(cloneValue(document)),
    };
    const cached = cache.get(key);
    if (cached !== undefined) {
      cache.delete(key);
      cache.set(key, cached);
      if (latestGeneration.get(chartPath) === generation) {
        latestGeneration.delete(chartPath);
      }
      return { status: "published" as const, value: cached };
    }

    let requestPromise!: Promise<ChartPreviewOutcome>;
    requestPromise = (async (): Promise<ChartPreviewOutcome> => {
      try {
        const value = await dependencies.service.query(
          identity.projectInstanceId,
          document.databaseId,
          chartPath,
          mutableDocument(document),
        );
        if (!current(identity, ownerCoordinatorEpoch, chartPath, generation)) {
          return { status: "stale" };
        }
        if (!isValidPreviewPayload(value)) {
          publishFailure(request, null);
          return { status: "failed" };
        }
        const frozen = freezeValue(cloneValue(value)) as DeepReadonly<ChartPreviewPayload>;
        if (!current(identity, ownerCoordinatorEpoch, chartPath, generation)) {
          return { status: "stale" };
        }
        try {
          dependencies.publication?.publish(request, frozen);
        } catch (error) {
          if (!current(identity, ownerCoordinatorEpoch, chartPath, generation)) {
            return { status: "stale" };
          }
          publishFailure(request, error);
          return { status: "failed" };
        }
        if (!current(identity, ownerCoordinatorEpoch, chartPath, generation)) {
          return { status: "stale" };
        }
        cache.set(key, frozen);
        while (cache.size > MAX_CACHE_ENTRIES) {
          const oldest = cache.keys().next().value as string | undefined;
          if (!oldest) break;
          cache.delete(oldest);
        }
        return { status: "published", value: frozen };
      } catch (error) {
        if (!current(identity, ownerCoordinatorEpoch, chartPath, generation)) {
          return { status: "stale" };
        }
        publishFailure(request, error);
        return { status: "failed" };
      } finally {
        if (inFlight.get(key) === requestPromise) inFlight.delete(key);
        if (latestGeneration.get(chartPath) === generation) {
          latestGeneration.delete(chartPath);
        }
      }
    })();

    inFlight.set(key, requestPromise);
    return requestPromise;
  };

  const resetProject = (): void => {
    coordinatorEpoch += 1;
    nextGeneration += 1;
    latestGeneration.clear();
    cache.clear();
    inFlight.clear();
    try {
      dependencies.publication?.clearForProject?.(captureIdentity()?.projectInstanceId ?? null);
    } catch {
      // Cache reset is authoritative even if an optional publication rejects it.
    }
  };

  return { query, resetProject, getCached };
}

function isValidPreviewPayload(value: unknown): value is ChartPreviewPayload {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ChartPreviewPayload>;
  if (candidate.kind === "empty") return true;
  if (candidate.kind === "error") {
    const error = candidate as Extract<ChartPreviewPayload, { kind: "error" }>;
    return (
      typeof error.code === "string" &&
      (error.incidentId === null || typeof error.incidentId === "string") &&
      (error.column === undefined || typeof error.column === "string")
    );
  }
  if (candidate.kind === "histogram") {
    const histogram = candidate as Extract<ChartPreviewPayload, { kind: "histogram" }>;
    return (
      Array.isArray(histogram.bins) &&
      histogram.bins.every((bin) => typeof bin.label === "string" && Number.isFinite(bin.count)) &&
      (histogram.xLabel === undefined || typeof histogram.xLabel === "string") &&
      (histogram.yLabel === undefined || typeof histogram.yLabel === "string")
    );
  }
  if (candidate.kind !== "scatter" && candidate.kind !== "line") return false;
  const plot = candidate as Extract<ChartPreviewPayload, { kind: "scatter" | "line" }>;
  return (
    !!plot.pair &&
    Array.isArray(plot.pair.data) &&
    plot.pair.data.every((point) => Number.isFinite(point.x) && Number.isFinite(point.y)) &&
    (plot.pair.xLabel === undefined || typeof plot.pair.xLabel === "string") &&
    (plot.pair.yLabel === undefined || typeof plot.pair.yLabel === "string") &&
    (plot.pair.xFormat === "date" ||
      plot.pair.xFormat === "datetime" ||
      plot.pair.xFormat === "number") &&
    (plot.pair.yFormat === "date" ||
      plot.pair.yFormat === "datetime" ||
      plot.pair.yFormat === "number")
  );
}
