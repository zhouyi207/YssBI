import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  getChartSnapshot,
  type PendingChartSave,
  type ChartReadSnapshot,
} from "@/features/core/chart/read";
import {
  optimisticOperationKey,
  chartDocumentFingerprint,
  chartProjectionPublication,
  chartSavePublication,
  type OptimisticOperationKey,
  type ChartProjectionPublication,
  type ChartSavePublication,
} from "@/features/core/chart/publication";
import {
  chartDraftReconciliation,
  type ChartDraftReconciliation,
} from "@/features/core/chart/reconciliation";
import { chartUi, type ChartUi } from "@/features/core/chart/ui";
import type { ChartDocument } from "@/shared/types/domain/chart";

export interface ChartProjectIdentity {
  readonly projectInstanceId: string;
  readonly epoch: number;
}

export interface ChartServicePort<TReceipt = unknown> {
  loadChart(projectInstanceId: string, chartPath: string): Promise<ChartDocument>;
  saveChart(
    projectInstanceId: string,
    operationId: string,
    chartPath: string,
    expectedRevision: number,
    document: ChartDocument,
  ): Promise<TReceipt>;
}

export type ChartSaveFailureKind = "rejected" | "unknown" | "failed";

export interface ChartSaveContext {
  readonly key: PendingChartSave;
  readonly document: DeepReadonly<ChartDocument>;
}

export interface ChartDocumentCoordinatorDependencies<TReceipt = unknown> {
  readonly captureProjectIdentity: () => ChartProjectIdentity | null;
  readonly service: ChartServicePort<TReceipt>;
  readonly projection?: ChartProjectionPublication;
  readonly savePublication?: ChartSavePublication;
  readonly draftReconciliation?: ChartDraftReconciliation;
  readonly ui?: ChartUi;
  readonly readSnapshot?: () => DeepReadonly<ChartReadSnapshot>;
  readonly publishCommittedReceipt?: (
    receipt: TReceipt,
    context: ChartSaveContext,
  ) => void | PromiseLike<void>;
  readonly requestAuthoritativeRecovery?: (key: OptimisticOperationKey) => void | PromiseLike<void>;
  readonly publishIssue?: (
    issue: ErrorReference,
    operation: "load" | "save",
  ) => void | PromiseLike<void>;
  readonly toErrorReference?: (error: unknown, fallbackCode: string) => ErrorReference;
  readonly classifySaveFailure?: (error: unknown) => ChartSaveFailureKind;
  readonly createOperationId?: () => string;
}

export type ChartLoadOutcome =
  | { readonly status: "loaded" }
  | { readonly status: "stale" }
  | { readonly status: "notReady" }
  | { readonly status: "failed" };

export type ChartSaveOutcome =
  | { readonly status: "acknowledged" }
  | { readonly status: "stale" }
  | { readonly status: "cancelled" }
  | { readonly status: "rejected" }
  | { readonly status: "unknown" }
  | { readonly status: "failed" };

export interface ChartDocumentCoordinator {
  load(chartPath: string): Promise<ChartLoadOutcome>;
  save(chartPath: string): Promise<ChartSaveOutcome>;
  discard(chartPath: string): void;
}

export type ChartCommittedDocumentOutcome = "applied" | "rebased" | "draft-changed" | "stale";

export interface ChartDocumentCoordinatorHandle extends ChartDocumentCoordinator {
  resetProject(): void;
  acceptCommittedDocument(
    chartPath: string,
    document: DeepReadonly<ChartDocument>,
    key?: OptimisticOperationKey,
  ): ChartCommittedDocumentOutcome;
}

interface RequestOwner {
  readonly identity: ChartProjectIdentity;
  readonly coordinatorEpoch: number;
  readonly requestGeneration: number;
}

const INVALID_CHART_RESULT = Symbol("invalid chart result");

function fallbackError(operation: "load" | "save"): ErrorReference {
  return {
    code: operation === "load" ? "chart_load_failed" : "chart_save_failed",
    incidentId: null,
  };
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

export function isChartDocument(value: unknown): value is ChartDocument {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<ChartDocument>;
  if (
    typeof candidate.schemaVersion !== "number" ||
    !Number.isSafeInteger(candidate.schemaVersion) ||
    candidate.schemaVersion < 0
  )
    return false;
  if (
    typeof candidate.revision !== "number" ||
    !Number.isSafeInteger(candidate.revision) ||
    candidate.revision < 0
  )
    return false;
  if (typeof candidate.databaseId !== "string") return false;
  if (
    candidate.chartType !== "histogram" &&
    candidate.chartType !== "scatter" &&
    candidate.chartType !== "line"
  )
    return false;
  if (!candidate.encodings || typeof candidate.encodings !== "object") return false;
  return (
    (candidate.encodings.x === undefined || typeof candidate.encodings.x === "string") &&
    (candidate.encodings.y === undefined || typeof candidate.encodings.y === "string")
  );
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

function defaultOperationId(): string {
  if (typeof globalThis.crypto?.randomUUID === "function") {
    return globalThis.crypto.randomUUID();
  }
  return `chart-operation-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function createChartDocumentCoordinator<TReceipt = unknown>(
  dependencies: ChartDocumentCoordinatorDependencies<TReceipt>,
): ChartDocumentCoordinatorHandle {
  const projection = dependencies.projection ?? chartProjectionPublication;
  const savePublication = dependencies.savePublication ?? chartSavePublication;
  const reconciliation = dependencies.draftReconciliation ?? chartDraftReconciliation;
  const ui = dependencies.ui ?? chartUi;
  const readSnapshot = dependencies.readSnapshot ?? getChartSnapshot;

  let coordinatorEpoch = 0;
  let nextRequestGeneration = 0;
  const latestLoadGeneration = new Map<string, number>();

  const captureIdentity = (): ChartProjectIdentity | null => {
    try {
      return dependencies.captureProjectIdentity();
    } catch {
      return null;
    }
  };

  const isCurrent = (owner: RequestOwner, chartPath?: string): boolean => {
    if (owner.coordinatorEpoch !== coordinatorEpoch) return false;
    if (chartPath !== undefined && latestLoadGeneration.get(chartPath) !== owner.requestGeneration)
      return false;
    const current = captureIdentity();
    return (
      current !== null &&
      current.projectInstanceId === owner.identity.projectInstanceId &&
      current.epoch === owner.identity.epoch
    );
  };

  const issueFor = (error: unknown, operation: "load" | "save"): ErrorReference => {
    try {
      const mapped = dependencies.toErrorReference?.(error, fallbackError(operation).code);
      if (isErrorReference(mapped)) return mapped;
    } catch {
      // Error conversion is advisory; the closed fallback is authoritative.
    }
    return fallbackError(operation);
  };

  const publishIssue = async (error: unknown, operation: "load" | "save"): Promise<void> => {
    try {
      await dependencies.publishIssue?.(issueFor(error, operation), operation);
    } catch {
      // Issue presentation cannot change the closed workflow outcome.
    }
  };

  const load = async (chartPath: string): Promise<ChartLoadOutcome> => {
    const identity = captureIdentity();
    if (!identity) return { status: "notReady" };

    const requestGeneration = ++nextRequestGeneration;
    latestLoadGeneration.set(chartPath, requestGeneration);
    const owner: RequestOwner = {
      identity,
      coordinatorEpoch,
      requestGeneration,
    };

    try {
      const loaded = await dependencies.service.loadChart(identity.projectInstanceId, chartPath);
      if (!isCurrent(owner, chartPath)) return { status: "stale" };
      if (!isChartDocument(loaded)) {
        await publishIssue(INVALID_CHART_RESULT, "load");
        return { status: "failed" };
      }
      projection.applyCommittedDocument(chartPath, loaded);
      return { status: "loaded" };
    } catch (error) {
      if (!isCurrent(owner, chartPath)) return { status: "stale" };
      await publishIssue(error, "load");
      return { status: "failed" };
    } finally {
      if (latestLoadGeneration.get(chartPath) === requestGeneration) {
        latestLoadGeneration.delete(chartPath);
      }
    }
  };

  const save = async (chartPath: string): Promise<ChartSaveOutcome> => {
    const identity = captureIdentity();
    if (!identity) {
      await publishIssue(null, "save");
      return { status: "failed" };
    }

    const snapshot = readSnapshot();
    const committed = snapshot.documents[chartPath];
    const draft = snapshot.draftsByPath[chartPath];
    if (!committed) {
      await publishIssue(null, "save");
      return { status: "failed" };
    }
    if (!draft || snapshot.dirtyByPath[chartPath] !== true) {
      return { status: "acknowledged" };
    }

    const operationId = dependencies.createOperationId?.() ?? defaultOperationId();
    const key: PendingChartSave = {
      projectInstanceId: identity.projectInstanceId,
      resourceKey: chartPath,
      operationId,
      fromRevision: committed.revision,
      draftFingerprint: chartDocumentFingerprint(draft),
      status: "pending",
    };
    const owner: RequestOwner = {
      identity,
      coordinatorEpoch,
      requestGeneration: ++nextRequestGeneration,
    };

    savePublication.beginPendingSave(key);
    try {
      const receipt = await dependencies.service.saveChart(
        identity.projectInstanceId,
        operationId,
        chartPath,
        committed.revision,
        mutableDocument(draft),
      );
      if (!isCurrent(owner)) return { status: "stale" };

      if (dependencies.publishCommittedReceipt) {
        try {
          await dependencies.publishCommittedReceipt(receipt, {
            key,
            document: draft,
          });
        } catch (error) {
          if (!isCurrent(owner)) return { status: "stale" };
          savePublication.markPendingSaveUnknown(key);
          try {
            await dependencies.requestAuthoritativeRecovery?.(key);
          } catch {
            // Unknown commit recovery remains required even if its request fails.
          }
          await publishIssue(error, "save");
          return { status: "unknown" };
        }
        if (!isCurrent(owner)) return { status: "stale" };
      }

      savePublication.markPendingSaveAcknowledged(key);
      return { status: "acknowledged" };
    } catch (error) {
      if (!isCurrent(owner)) return { status: "stale" };
      let failure: ChartSaveFailureKind = "unknown";
      try {
        failure = dependencies.classifySaveFailure?.(error) ?? "unknown";
      } catch {
        failure = "unknown";
      }

      if (failure === "rejected") {
        savePublication.settlePendingSave(key);
        await publishIssue(error, "save");
        return { status: "rejected" };
      }
      if (failure === "failed") {
        savePublication.settlePendingSave(key);
        await publishIssue(error, "save");
        return { status: "failed" };
      }

      savePublication.markPendingSaveUnknown(key);
      try {
        await dependencies.requestAuthoritativeRecovery?.(key);
      } catch {
        // Unknown commit recovery remains required even if its request fails.
      }
      await publishIssue(error, "save");
      return { status: "unknown" };
    }
  };

  const discard = (chartPath: string): void => {
    ui.discardDraft(chartPath);
  };

  const resetProject = (): void => {
    coordinatorEpoch += 1;
    nextRequestGeneration += 1;
    latestLoadGeneration.clear();
    const identity = captureIdentity();
    projection.clearForProject(identity?.projectInstanceId ?? null);
    savePublication.clearPendingSaves(null);
  };

  const acceptCommittedDocument = (
    chartPath: string,
    document: DeepReadonly<ChartDocument>,
    key?: OptimisticOperationKey,
  ): ChartCommittedDocumentOutcome => {
    const identity = captureIdentity();
    if (!identity) return "stale";
    if (
      key &&
      (key.projectInstanceId !== identity.projectInstanceId || key.resourceKey !== chartPath)
    ) {
      return "stale";
    }

    const pending = key
      ? readSnapshot().pendingSaveByPath[key.resourceKey]?.[optimisticOperationKey(key)]
      : undefined;
    projection.applyCommittedDocument(chartPath, document);
    if (!key || !pending) return "applied";

    const draftOutcome = reconciliation.rebaseCommittedDraft(
      chartPath,
      document,
      pending.draftFingerprint,
    );
    savePublication.settlePendingSave(key);
    return draftOutcome;
  };

  return {
    load,
    save,
    discard,
    resetProject,
    acceptCommittedDocument,
  };
}
