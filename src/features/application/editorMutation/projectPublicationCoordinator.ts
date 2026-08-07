import type { EditorGraphProjectionDto } from '@/shared/types/dto/editorProjection';
import type {
  FunctionSignatureDto,
  GraphProjectionReplacementDto,
  HistoryStatusDto,
  ResourceMoveDto,
  ResourceMutationResultDto,
} from '@/shared/types/dto/editorMutation';
import type {
  DatabaseRecord,
  FunctionSignaturePin,
  Variable,
  WorksheetDocument,
  WorksheetIndexEntry,
} from '@/shared/types';
import type { PreparedGraphProjectionReplacements } from '@/features/core/dataStore/graphDataStore';
import {
  acceptProjectLifecycleActivation,
  captureProjectIdentity,
  captureProjectLifecycleState,
  clearProjectLifecycle,
  isCurrentProjectIdentity,
  startProjectLifecycle,
  type ProjectIdentitySnapshot,
} from '@/features/core/projectLifecycle/projectLifecycleAuthority';
import type { EditorTabMemento } from '@/features/core/layout/editorTabStore';
import type { GraphMeta } from '@/features/core/dataStore/graphMetaStore';
import type { FocusedGraphSession } from '@/features/core/graphSession/graphSessionStore';
import type { EditorViewport } from '@/features/core/viewport/editorViewport';
import type { DocumentState, ProjectResourceMeta, ResourceKey } from '@/features/core/resource';
import { toProjectionEntities } from '@/features/domain/editorProjection';
import { ProjectService, type ProjectIndexRow } from '@/services/project/projectService';
import { clearWorksheetPreviewCache } from '@/services/worksheet/worksheetPreviewCache';
import { prepareGraphProjectionForPublication } from '@/features/application/editorProjection/graphProjectionCoordinator';
import { useGraphDataStore } from '@/features/core/dataStore/graphDataStore';
import { useHistoryStore } from '@/features/core/history';
import { useDocumentStateStore, useResourceStore } from '@/features/core/resource';
import {
  collectResourceMutationGraphPaths,
  commitPreparedPublication,
  fingerprintResourceMutationResult,
  prepareSynchronousPublicationCommit,
  validateResourceMutationWireResult,
} from './resourceMutationResult';
import {
  prepareGraphResourceMove,
  type PreparedGraphResourceMove,
} from './projectPublicationMovePlan';
import {
  buildProjectRecoveryPathRemaps,
  collectProjectRecoveryGraphPaths,
  commitPreparedProjectRecovery,
  prepareProjectRecoveryCommit,
  validateProjectRecoveryIndex,
} from './projectPublicationRecovery';

export type ProjectPublicationSuccess =
  | { status: 'applied'; affectedGraphPaths: ReadonlySet<string> }
  | { status: 'duplicate'; affectedGraphPaths: ReadonlySet<string> }
  | { status: 'recovered'; affectedGraphPaths: ReadonlySet<string> };

export type ProjectPublicationErrorCode =
  | 'stale_project_lifecycle'
  | 'publication_protocol_error'
  | 'publication_recovery_failed';

export class ProjectPublicationError extends Error {
  constructor(
    readonly code: ProjectPublicationErrorCode,
    message: string,
    options?: { cause?: unknown },
  ) {
    super(message);
    this.name = 'ProjectPublicationError';
    if (options && 'cause' in options) {
      (this as Error & { cause?: unknown }).cause = options.cause;
    }
  }
}

export interface ProjectPublicationSubmission {
  result: ResourceMutationResultDto;
  fallbackPaths?: readonly string[];
  validate?: (result: ResourceMutationResultDto) => string | undefined;
}

export interface PreparedFunctionDeltaInstall {
  readonly graphPath: string;
  readonly revision: number;
  readonly signature: FunctionSignatureDto;
  readonly functionInputs: readonly FunctionSignaturePin[];
  readonly functionOutputs: readonly FunctionSignaturePin[];
}

export interface PreparedVariableDeltaInstall {
  readonly id: string;
  readonly before: Variable | null;
  readonly after: Variable | null;
  readonly fromRevision: number;
  readonly toRevision: number;
}

export interface PreparedWorksheetDeltaInstall {
  readonly id: string;
  readonly before: WorksheetDocument | null;
  readonly after: WorksheetDocument | null;
}

export interface PreparedWorksheetPublicationState {
  readonly index: readonly WorksheetIndexEntry[];
  readonly documents: Readonly<Record<string, WorksheetDocument>>;
  readonly resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>;
  readonly documentStates: Readonly<Record<ResourceKey, DocumentState>>;
  readonly tabs: EditorTabMemento;
}

export interface PreparedPublicationStoreState {
  readonly resources: Readonly<Record<ResourceKey, ProjectResourceMeta>>;
  readonly graphOrder: string[];
  readonly documents: Readonly<Record<ResourceKey, DocumentState>>;
  readonly graphMeta: Readonly<Record<string, GraphMeta>>;
  readonly databases: Readonly<Record<string, DatabaseRecord>>;
  readonly databaseRevisions: Readonly<Record<string, number>>;
  readonly variables: Readonly<Record<string, Variable>>;
  readonly variableRevisions: Readonly<Record<string, number>>;
  readonly worksheetIndex: WorksheetIndexEntry[];
  readonly worksheetDocuments: Readonly<Record<string, WorksheetDocument>>;
  readonly tabs: EditorTabMemento;
  readonly focusedSession: FocusedGraphSession | null;
  readonly viewports: Readonly<Record<string, EditorViewport>>;
}

export interface PreparePublicationContext {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly fingerprint: string;
  readonly affectedGraphPaths: ReadonlySet<string>;
  readonly moves: readonly PreparedGraphResourceMove[];
}

export interface PreparedProjectPublication {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly publicationRevision: number;
  readonly fingerprint: string;
  readonly affectedGraphPaths: ReadonlySet<string>;
  readonly moves: readonly PreparedGraphResourceMove[];
  readonly graphProjectionPlan: PreparedGraphProjectionReplacements;
  readonly projectionReplacements: readonly GraphProjectionReplacementDto[];
  readonly functionInstalls: readonly PreparedFunctionDeltaInstall[];
  readonly variableInstalls: readonly PreparedVariableDeltaInstall[];
  readonly worksheetInstalls: readonly PreparedWorksheetDeltaInstall[];
  readonly worksheetState: PreparedWorksheetPublicationState;
  readonly storeState: PreparedPublicationStoreState;
  readonly history: HistoryStatusDto;
}

export interface ProjectRecoveryPreparation {
  readonly projectInstanceId: string;
  readonly epoch: number;
  readonly publicationRevision: number;
  readonly index: ProjectIndexRow;
  readonly projections: ReadonlyMap<string, EditorGraphProjectionDto>;
  readonly graphPathsLoadedAtStart: ReadonlySet<string>;
  readonly pathRemaps: ReadonlyMap<string, string>;
}

export interface PreparedProjectRecovery extends ProjectRecoveryPreparation {
  readonly graphProjectionPlan: PreparedGraphProjectionReplacements;
  readonly storeState: PreparedPublicationStoreState;
  readonly history: HistoryStatusDto;
}

export interface ProjectPublicationDependencies {
  loadRecoverySnapshot(projectInstanceId: string): Promise<ProjectIndexRow>;
  prepareGraphProjection(
    graphPath: string,
    projectInstanceId: string,
    epoch: number,
  ): Promise<EditorGraphProjectionDto | false>;
  captureLoadedGraphPaths(): ReadonlySet<string>;
  preparePublication(
    result: ResourceMutationResultDto,
    context: PreparePublicationContext,
  ): PreparedProjectPublication;
  prepareRecovery(plan: ProjectRecoveryPreparation): PreparedProjectRecovery;
  prepareMove(
    move: ResourceMoveDto,
    hasAuthoritativeDestinationReplacement: boolean,
  ): PreparedGraphResourceMove;
  commitPublication(plan: PreparedProjectPublication): void;
  commitRecovery(plan: PreparedProjectRecovery): void;
  markProjectProjectionStale(): void;
}

interface PublicationWaiter {
  resolve(value: ProjectPublicationSuccess): void;
  reject(reason: ProjectPublicationError): void;
}

interface PendingPublication {
  readonly revision: number;
  readonly fingerprint: string;
  readonly input: ProjectPublicationSubmission;
  readonly affectedGraphPaths: ReadonlySet<string>;
  readonly waiters: PublicationWaiter[];
  ownerRecoveryAttempt?: number;
  requiresRecovery?: boolean;
}

interface ProjectPublicationState {
  appliedRevision: number;
  appliedFingerprint?: string;
  phase: 'idle' | 'applying' | 'recovering';
  pendingByRevision: Map<number, PendingPublication>;
}



function protocolError(message: string, cause?: unknown): ProjectPublicationError {
  return new ProjectPublicationError('publication_protocol_error', message, { cause });
}

function recoveryError(message: string, cause?: unknown): ProjectPublicationError {
  return new ProjectPublicationError('publication_recovery_failed', message, { cause });
}

function staleLifecycleError(): ProjectPublicationError {
  return new ProjectPublicationError(
    'stale_project_lifecycle',
    'project lifecycle changed before publication settlement',
  );
}



export class ProjectPublicationCoordinator {
  private readonly state: ProjectPublicationState = {
    appliedRevision: 0,
    phase: 'idle',
    pendingByRevision: new Map(),
  };

  private recoveryAttempt = 0;
  private recoveryVersion = 0;
  private activeRecoverySnapshotRevision: number | null = null;
  private driverInFlight: Promise<void> | null = null;

  constructor(private readonly dependencies: ProjectPublicationDependencies) {}

  validateProjectStart(projectInstanceId: string, appliedRevision: number): void {
    if (!projectInstanceId || !Number.isSafeInteger(appliedRevision) || appliedRevision < 0) {
      throw protocolError('project publication baseline is malformed');
    }
  }

  startProject(projectInstanceId: string, appliedRevision: number): void {
    this.validateProjectStart(projectInstanceId, appliedRevision);
    clearWorksheetPreviewCache();
    startProjectLifecycle(projectInstanceId);
    this.resetPublicationState(appliedRevision);
  }

  acceptProjectActivation(projectInstanceId: string, activationRevision: number): boolean {
    if (!projectInstanceId
      || !Number.isSafeInteger(activationRevision)
      || activationRevision <= 0) {
      throw protocolError('project activation identity is malformed');
    }
    const result = acceptProjectLifecycleActivation(projectInstanceId, activationRevision);
    if (result === 'stale') return false;
    if (result === 'activated') {
      clearWorksheetPreviewCache();
      this.resetPublicationState(0);
    }
    return true;
  }

  cancelProject(): void {
    clearWorksheetPreviewCache();
    clearProjectLifecycle();
    this.resetPublicationState(0);
  }

  submit(input: ProjectPublicationSubmission): Promise<ProjectPublicationSuccess> {
    const validation = this.validateSubmission(input);
    if (validation) return Promise.reject(validation);

    const revision = input.result.publicationRevision;
    const fingerprint = fingerprintResourceMutationResult(input.result);
    const affectedGraphPaths = collectResourceMutationGraphPaths(
      input.result,
      input.fallbackPaths ?? [],
    );
    const existing = this.state.pendingByRevision.get(revision);
    if (existing) {
      if (existing.fingerprint !== fingerprint) {
        return Promise.reject(protocolError(
          `publication revision ${revision} conflicts with a different result`,
        ));
      }
      return this.addWaiter(existing);
    }

    if (this.state.phase === 'recovering') {
      const pending = this.createPending(input, fingerprint, affectedGraphPaths);
      const snapshotRevision = this.activeRecoverySnapshotRevision;
      if (snapshotRevision !== null && revision <= snapshotRevision) {
        pending.ownerRecoveryAttempt = this.recoveryAttempt;
        this.recoveryVersion += 1;
      }
      this.state.pendingByRevision.set(revision, pending);
      return this.addWaiter(pending);
    }

    if (revision === this.state.appliedRevision
      && fingerprint === this.state.appliedFingerprint) {
      return Promise.resolve({ status: 'duplicate', affectedGraphPaths });
    }
    if (revision <= this.state.appliedRevision) {
      const pending = this.createPending(input, fingerprint, affectedGraphPaths);
      this.state.pendingByRevision.set(revision, pending);
      const promise = this.addWaiter(pending);
      this.kick();
      return promise;
    }

    const pending = this.createPending(input, fingerprint, affectedGraphPaths);
    this.state.pendingByRevision.set(revision, pending);
    const promise = this.addWaiter(pending);
    this.kick();
    return promise;
  }

  capturePublicationRevision(): number {
    return this.state.appliedRevision;
  }

  captureCommandLifecycle(): {
    projectInstanceId: string;
    epoch: number;
    publicationRevision: number;
  } {
    const identity = captureProjectIdentity();
    const publicationRevision = this.capturePublicationRevision();
    this.assertLifecycle(identity.projectInstanceId, identity.epoch);
    return { ...identity, publicationRevision };
  }

  markProjectProjectionStale(): void {
    this.dependencies.markProjectProjectionStale();
  }

  getSnapshotForTests(): {
    projectInstanceId: string | null;
    epoch: number;
    appliedRevision: number;
    phase: 'idle' | 'applying' | 'recovering';
    pendingRevisions: number[];
  } {
    const lifecycle = captureProjectLifecycleState();
    return {
      ...lifecycle,
      appliedRevision: this.state.appliedRevision,
      phase: this.state.phase,
      pendingRevisions: [...this.state.pendingByRevision.keys()].sort((a, b) => a - b),
    };
  }

  private validateSubmission(input: ProjectPublicationSubmission): ProjectPublicationError | undefined {
    const lifecycle = captureProjectLifecycleState();
    if (!lifecycle.projectInstanceId) return staleLifecycleError();
    if (input.result.projectInstanceId !== lifecycle.projectInstanceId) return staleLifecycleError();
    const wireError = validateResourceMutationWireResult(input.result);
    if (wireError) return protocolError(wireError);
    const callerError = input.validate?.(input.result);
    if (callerError) return protocolError(callerError);
    return undefined;
  }

  private createPending(
    input: ProjectPublicationSubmission,
    fingerprint: string,
    affectedGraphPaths: ReadonlySet<string>,
  ): PendingPublication {
    return {
      revision: input.result.publicationRevision,
      fingerprint,
      input,
      affectedGraphPaths,
      waiters: [],
    };
  }

  private addWaiter(pending: PendingPublication): Promise<ProjectPublicationSuccess> {
    return new Promise((resolve, reject) => pending.waiters.push({ resolve, reject }));
  }

  private resetPublicationState(appliedRevision: number): void {
    const error = staleLifecycleError();
    for (const pending of this.state.pendingByRevision.values()) {
      for (const waiter of pending.waiters) waiter.reject(error);
    }
    this.state.pendingByRevision.clear();
    this.state.appliedRevision = appliedRevision;
    this.state.appliedFingerprint = undefined;
    this.state.phase = 'idle';
    this.activeRecoverySnapshotRevision = null;
    this.driverInFlight = null;
  }

  private ownsLifecycle(projectInstanceId: string, epoch: number): boolean {
    return isCurrentProjectIdentity({ projectInstanceId, epoch });
  }

  private assertLifecycle(projectInstanceId: string, epoch: number): void {
    if (!this.ownsLifecycle(projectInstanceId, epoch)) throw staleLifecycleError();
  }

  private kick(): void {
    if (this.driverInFlight) return;
    let identity: ProjectIdentitySnapshot;
    try {
      identity = captureProjectIdentity();
    } catch {
      return;
    }
    const driver = this.drive(identity);
    this.driverInFlight = driver;
    void driver.finally(() => {
      if (this.driverInFlight !== driver) return;
      this.driverInFlight = null;
      if (isCurrentProjectIdentity(identity)) this.state.phase = 'idle';
      if (this.state.pendingByRevision.size > 0) this.kick();
    }).catch(() => undefined);
  }

  private async drive(identity: ProjectIdentitySnapshot): Promise<void> {
    while (isCurrentProjectIdentity(identity) && this.state.pendingByRevision.size > 0) {
      const next = this.state.pendingByRevision.get(this.state.appliedRevision + 1);
      if (next && !next.requiresRecovery) {
        await this.applyPending(next);
      } else {
        await this.runRecovery();
      }
    }
  }

  private async prepareProjection(
    graphPath: string,
    projectInstanceId: string,
    epoch: number,
  ): Promise<EditorGraphProjectionDto> {
    const projection = await this.dependencies.prepareGraphProjection(
      graphPath,
      projectInstanceId,
      epoch,
    );
    this.assertLifecycle(projectInstanceId, epoch);
    if (projection === false) throw new Error(`projection preparation failed for '${graphPath}'`);
    const entities = toProjectionEntities(projection);
    if (entities.graphPath !== graphPath
      || projection.graphPath !== graphPath
      || projection.basis?.graphPath !== graphPath) {
      throw new Error(`prepared projection identity is invalid for '${graphPath}'`);
    }
    return projection;
  }

  private async applyPending(pending: PendingPublication): Promise<void> {
    let identity: ProjectIdentitySnapshot;
    try {
      identity = captureProjectIdentity();
    } catch {
      return;
    }
    this.state.phase = 'applying';
    const { projectInstanceId, epoch } = identity;
    try {
      const result = pending.input.result;
      if (result.projectionStatus.status === 'incomplete') {
        pending.requiresRecovery = true;
        return;
      }
      const replacementPaths = new Set(
        result.projectionReplacements.map((replacement) => replacement.graphPath),
      );
      const moves = result.moves.map((move) =>
        this.dependencies.prepareMove(move, replacementPaths.has(move.to)));
      this.assertLifecycle(projectInstanceId, epoch);
      if (pending.revision !== this.state.appliedRevision + 1) return;
      const plan = this.dependencies.preparePublication(result, {
        projectInstanceId,
        epoch,
        fingerprint: pending.fingerprint,
        affectedGraphPaths: pending.affectedGraphPaths,
        moves,
      });
      this.dependencies.commitPublication(plan);
      this.assertLifecycle(projectInstanceId, epoch);
      this.state.appliedFingerprint = pending.fingerprint;
      this.state.appliedRevision = pending.revision;
      this.state.pendingByRevision.delete(pending.revision);
      for (const waiter of pending.waiters) {
        waiter.resolve({ status: 'applied', affectedGraphPaths: pending.affectedGraphPaths });
      }
    } catch (error) {
      if (error instanceof ProjectPublicationError && error.code === 'stale_project_lifecycle') return;
      pending.requiresRecovery = true;
    }
  }

  private async runRecovery(): Promise<void> {
    if (this.state.pendingByRevision.size === 0) return;
    let identity: ProjectIdentitySnapshot;
    try {
      identity = captureProjectIdentity();
    } catch {
      return;
    }
    const attempt = ++this.recoveryAttempt;
    const { projectInstanceId, epoch } = identity;
    const loadedAtStart = this.dependencies.captureLoadedGraphPaths();
    const coveredAtStart = new Set(this.state.pendingByRevision.values());
    this.state.phase = 'recovering';
    try {
      await this.recover(
        attempt,
        projectInstanceId,
        epoch,
        loadedAtStart,
        coveredAtStart,
      );
    } finally {
      if (this.ownsLifecycle(projectInstanceId, epoch) && this.recoveryAttempt === attempt) {
        this.activeRecoverySnapshotRevision = null;
      }
    }
  }

  private async recover(
    attempt: number,
    projectInstanceId: string,
    epoch: number,
    graphPathsLoadedAtStart: ReadonlySet<string>,
    coveredAtStart: ReadonlySet<PendingPublication>,
  ): Promise<void> {
    let snapshotRevision: number | null = null;
    let rejectCoveredAtStart = false;
    try {
      const index = await this.dependencies.loadRecoverySnapshot(projectInstanceId);
      this.assertLifecycle(projectInstanceId, epoch);
      const indexError = validateProjectRecoveryIndex(index, projectInstanceId);
      if (indexError) throw new Error(indexError);

      const validatedSnapshotRevision = index.publicationRevision;
      snapshotRevision = validatedSnapshotRevision;
      this.activeRecoverySnapshotRevision = validatedSnapshotRevision;
      for (const pending of this.state.pendingByRevision.values()) {
        if (pending.revision <= validatedSnapshotRevision) pending.ownerRecoveryAttempt = attempt;
      }
      const initiallyOwned = [...this.state.pendingByRevision.values()]
        .filter((pending) => pending.ownerRecoveryAttempt === attempt
          && pending.revision <= validatedSnapshotRevision);
      const nextContiguous = this.state.pendingByRevision.get(this.state.appliedRevision + 1);
      if (validatedSnapshotRevision === this.state.appliedRevision
        && initiallyOwned.length === 0
        && nextContiguous
        && !nextContiguous.requiresRecovery) {
        return;
      }
      if (validatedSnapshotRevision < this.state.appliedRevision
        || (validatedSnapshotRevision === this.state.appliedRevision
          && initiallyOwned.length === 0)) {
        rejectCoveredAtStart = true;
        throw new Error(
          `recovery snapshot revision ${validatedSnapshotRevision} does not advance or cover the attempt`,
        );
      }

      const authoritativeGraphPaths = new Set(index.graphs.map((graph) => graph.path));
      const projections = new Map<string, EditorGraphProjectionDto>();
      let owned: PendingPublication[] = initiallyOwned;
      let queuedResults: ResourceMutationResultDto[] = [];
      let pathRemaps: ReadonlyMap<string, string> = new Map();
      for (;;) {
        const observedVersion = this.recoveryVersion;
        owned = [...this.state.pendingByRevision.values()]
          .filter((pending) => pending.ownerRecoveryAttempt === attempt
            && pending.revision <= validatedSnapshotRevision);
        queuedResults = owned.map((pending) => pending.input.result);
        pathRemaps = buildProjectRecoveryPathRemaps(authoritativeGraphPaths, queuedResults);
        const recoveryPaths = collectProjectRecoveryGraphPaths(
          index,
          graphPathsLoadedAtStart,
          queuedResults,
        );
        for (const path of recoveryPaths) {
          if (projections.has(path)) continue;
          projections.set(path, await this.prepareProjection(path, projectInstanceId, epoch));
        }
        this.assertLifecycle(projectInstanceId, epoch);
        if (observedVersion === this.recoveryVersion) break;
      }
      const recoveryPreparation: ProjectRecoveryPreparation = {
        projectInstanceId,
        epoch,
        publicationRevision: index.publicationRevision,
        index,
        projections,
        graphPathsLoadedAtStart,
        pathRemaps,
      };
      const plan = this.dependencies.prepareRecovery(recoveryPreparation);
      this.dependencies.commitRecovery(plan);
      this.assertLifecycle(projectInstanceId, epoch);
      this.state.appliedRevision = index.publicationRevision;
      this.state.appliedFingerprint = undefined;
      for (const pending of owned) {
        if (pending.revision > index.publicationRevision) continue;
        this.state.pendingByRevision.delete(pending.revision);
        for (const waiter of pending.waiters) {
          waiter.resolve({ status: 'recovered', affectedGraphPaths: pending.affectedGraphPaths });
        }
      }
    } catch (cause) {
      if (!this.ownsLifecycle(projectInstanceId, epoch)) return;
      if (cause instanceof ProjectPublicationError && cause.code === 'stale_project_lifecycle') return;
      const error = recoveryError('authoritative project publication recovery failed', cause);
      for (const [revision, pending] of this.state.pendingByRevision) {
        const covered = snapshotRevision === null || rejectCoveredAtStart
          ? coveredAtStart.has(pending)
          : pending.ownerRecoveryAttempt === attempt && pending.revision <= snapshotRevision;
        if (!covered) continue;
        this.state.pendingByRevision.delete(revision);
        for (const waiter of pending.waiters) waiter.reject(error);
      }
      this.state.phase = 'idle';
      this.dependencies.markProjectProjectionStale();
    }
  }
}

const productionDependencies: ProjectPublicationDependencies = {
  loadRecoverySnapshot: (projectInstanceId) => ProjectService.getProjectIndex(projectInstanceId),
  prepareGraphProjection: prepareGraphProjectionForPublication,
  captureLoadedGraphPaths: () => new Set(Object.keys(useGraphDataStore.getState().graphEntities)),
  preparePublication: prepareSynchronousPublicationCommit,
  prepareRecovery: prepareProjectRecoveryCommit,
  prepareMove: prepareGraphResourceMove,
  commitPublication: (plan) => {
    commitPreparedPublication(plan);
    useHistoryStore.setState({
      canUndo: plan.history.canUndo,
      canRedo: plan.history.canRedo,
    });
  },
  commitRecovery: commitPreparedProjectRecovery,
  markProjectProjectionStale: () => {
    useResourceStore.setState((state) => ({
      resources: Object.fromEntries(Object.entries(state.resources).map(([key, resource]) => [
        key,
        resource.kind === 'event' || resource.kind === 'function'
          ? { ...resource, hasStaleDocument: true }
          : resource,
      ])),
    }));
    useDocumentStateStore.setState((state) => ({
      documents: Object.fromEntries(Object.entries(state.documents).map(([key, document]) => [
        key,
        { ...document, stale: true },
      ])),
    }));
  },
};

export const projectPublicationCoordinator = new ProjectPublicationCoordinator(productionDependencies);
