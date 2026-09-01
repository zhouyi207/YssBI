import type { ErrorReference } from "@/features/application/errorReference";
import type { DeepReadonly } from "@/shared/types/deepReadonly";
import {
  getWorksheetSnapshot,
  type PendingWorksheetSave,
  type WorksheetReadSnapshot,
} from "@/features/core/worksheet/read";
import {
  optimisticOperationKey,
  worksheetDocumentFingerprint,
  worksheetProjectionPublication,
  worksheetSavePublication,
  type OptimisticOperationKey,
  type WorksheetProjectionPublication,
  type WorksheetSavePublication,
} from "@/features/core/worksheet/publication";
import {
  worksheetDraftReconciliation,
  type WorksheetDraftReconciliation,
} from "@/features/core/worksheet/reconciliation";
import { worksheetUi, type WorksheetUi } from "@/features/core/worksheet/ui";
import type { WorksheetDocument } from "@/shared/types/domain/worksheet";

export interface WorksheetProjectIdentity {
  readonly projectInstanceId: string;
  readonly epoch: number;
}

export interface WorksheetServicePort<TReceipt = unknown> {
  loadWorksheet(projectInstanceId: string, worksheetPath: string): Promise<WorksheetDocument>;
  saveWorksheet(
    projectInstanceId: string,
    operationId: string,
    worksheetPath: string,
    expectedRevision: number,
    document: WorksheetDocument,
  ): Promise<TReceipt>;
}

export type WorksheetSaveFailureKind = "rejected" | "unknown" | "failed";

export interface WorksheetSaveContext {
  readonly key: PendingWorksheetSave;
  readonly document: DeepReadonly<WorksheetDocument>;
}

export interface WorksheetCoordinatorDependencies<TReceipt = unknown> {
  readonly captureProjectIdentity: () => WorksheetProjectIdentity | null;
  readonly service: WorksheetServicePort<TReceipt>;
  readonly projection?: WorksheetProjectionPublication;
  readonly savePublication?: WorksheetSavePublication;
  readonly draftReconciliation?: WorksheetDraftReconciliation;
  readonly ui?: WorksheetUi;
  readonly readSnapshot?: () => DeepReadonly<WorksheetReadSnapshot>;
  readonly publishCommittedReceipt?: (
    receipt: TReceipt,
    context: WorksheetSaveContext,
  ) => void | PromiseLike<void>;
  readonly requestAuthoritativeRecovery?: (key: OptimisticOperationKey) => void | PromiseLike<void>;
  readonly publishIssue?: (
    issue: ErrorReference,
    operation: "load" | "save",
  ) => void | PromiseLike<void>;
  readonly toErrorReference?: (error: unknown, fallbackCode: string) => ErrorReference;
  readonly classifySaveFailure?: (error: unknown) => WorksheetSaveFailureKind;
  readonly createOperationId?: () => string;
}

export type WorksheetLoadOutcome =
  | { readonly status: "loaded" }
  | { readonly status: "stale" }
  | { readonly status: "notReady" }
  | { readonly status: "failed" };

export type WorksheetSaveOutcome =
  | { readonly status: "acknowledged" }
  | { readonly status: "stale" }
  | { readonly status: "cancelled" }
  | { readonly status: "rejected" }
  | { readonly status: "unknown" }
  | { readonly status: "failed" };

export interface WorksheetCoordinator {
  load(worksheetPath: string): Promise<WorksheetLoadOutcome>;
  save(worksheetPath: string): Promise<WorksheetSaveOutcome>;
  discard(worksheetPath: string): void;
}

export type WorksheetCommittedDocumentOutcome = "applied" | "rebased" | "draft-changed" | "stale";

export interface WorksheetCoordinatorHandle extends WorksheetCoordinator {
  resetProject(): void;
  acceptCommittedDocument(
    worksheetPath: string,
    document: DeepReadonly<WorksheetDocument>,
    key?: OptimisticOperationKey,
  ): WorksheetCommittedDocumentOutcome;
}

interface RequestOwner {
  readonly identity: WorksheetProjectIdentity;
  readonly coordinatorEpoch: number;
  readonly requestGeneration: number;
}

const INVALID_WORKSHEET_RESULT = Symbol("invalid worksheet result");

function fallbackError(operation: "load" | "save"): ErrorReference {
  return {
    code: operation === "load" ? "worksheet_load_failed" : "worksheet_save_failed",
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

export function isWorksheetDocument(value: unknown): value is WorksheetDocument {
  if (!value || typeof value !== "object") return false;
  const candidate = value as Partial<WorksheetDocument>;
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

function mutableDocument(document: DeepReadonly<WorksheetDocument>): WorksheetDocument {
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
  return `worksheet-operation-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

export function createWorksheetCoordinator<TReceipt = unknown>(
  dependencies: WorksheetCoordinatorDependencies<TReceipt>,
): WorksheetCoordinatorHandle {
  const projection = dependencies.projection ?? worksheetProjectionPublication;
  const savePublication = dependencies.savePublication ?? worksheetSavePublication;
  const reconciliation = dependencies.draftReconciliation ?? worksheetDraftReconciliation;
  const ui = dependencies.ui ?? worksheetUi;
  const readSnapshot = dependencies.readSnapshot ?? getWorksheetSnapshot;

  let coordinatorEpoch = 0;
  let nextRequestGeneration = 0;
  const latestLoadGeneration = new Map<string, number>();

  const captureIdentity = (): WorksheetProjectIdentity | null => {
    try {
      return dependencies.captureProjectIdentity();
    } catch {
      return null;
    }
  };

  const isCurrent = (owner: RequestOwner, worksheetPath?: string): boolean => {
    if (owner.coordinatorEpoch !== coordinatorEpoch) return false;
    if (
      worksheetPath !== undefined &&
      latestLoadGeneration.get(worksheetPath) !== owner.requestGeneration
    )
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

  const load = async (worksheetPath: string): Promise<WorksheetLoadOutcome> => {
    const identity = captureIdentity();
    if (!identity) return { status: "notReady" };

    const requestGeneration = ++nextRequestGeneration;
    latestLoadGeneration.set(worksheetPath, requestGeneration);
    const owner: RequestOwner = {
      identity,
      coordinatorEpoch,
      requestGeneration,
    };

    try {
      const loaded = await dependencies.service.loadWorksheet(
        identity.projectInstanceId,
        worksheetPath,
      );
      if (!isCurrent(owner, worksheetPath)) return { status: "stale" };
      if (!isWorksheetDocument(loaded)) {
        await publishIssue(INVALID_WORKSHEET_RESULT, "load");
        return { status: "failed" };
      }
      projection.applyCommittedDocument(worksheetPath, loaded);
      return { status: "loaded" };
    } catch (error) {
      if (!isCurrent(owner, worksheetPath)) return { status: "stale" };
      await publishIssue(error, "load");
      return { status: "failed" };
    } finally {
      if (latestLoadGeneration.get(worksheetPath) === requestGeneration) {
        latestLoadGeneration.delete(worksheetPath);
      }
    }
  };

  const save = async (worksheetPath: string): Promise<WorksheetSaveOutcome> => {
    const identity = captureIdentity();
    if (!identity) {
      await publishIssue(null, "save");
      return { status: "failed" };
    }

    const snapshot = readSnapshot();
    const committed = snapshot.documents[worksheetPath];
    const draft = snapshot.draftsByPath[worksheetPath];
    if (!committed) {
      await publishIssue(null, "save");
      return { status: "failed" };
    }
    if (!draft || snapshot.dirtyByPath[worksheetPath] !== true) {
      return { status: "acknowledged" };
    }

    const operationId = dependencies.createOperationId?.() ?? defaultOperationId();
    const key: PendingWorksheetSave = {
      projectInstanceId: identity.projectInstanceId,
      resourceKey: worksheetPath,
      operationId,
      fromRevision: committed.revision,
      draftFingerprint: worksheetDocumentFingerprint(draft),
      status: "pending",
    };
    const owner: RequestOwner = {
      identity,
      coordinatorEpoch,
      requestGeneration: ++nextRequestGeneration,
    };

    savePublication.beginPendingSave(key);
    try {
      const receipt = await dependencies.service.saveWorksheet(
        identity.projectInstanceId,
        operationId,
        worksheetPath,
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
      let failure: WorksheetSaveFailureKind = "unknown";
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

  const discard = (worksheetPath: string): void => {
    ui.discardDraft(worksheetPath);
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
    worksheetPath: string,
    document: DeepReadonly<WorksheetDocument>,
    key?: OptimisticOperationKey,
  ): WorksheetCommittedDocumentOutcome => {
    const identity = captureIdentity();
    if (!identity) return "stale";
    if (
      key &&
      (key.projectInstanceId !== identity.projectInstanceId || key.resourceKey !== worksheetPath)
    ) {
      return "stale";
    }

    const pending = key
      ? readSnapshot().pendingSaveByPath[key.resourceKey]?.[optimisticOperationKey(key)]
      : undefined;
    projection.applyCommittedDocument(worksheetPath, document);
    if (!key || !pending) return "applied";

    const draftOutcome = reconciliation.rebaseCommittedDraft(
      worksheetPath,
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
