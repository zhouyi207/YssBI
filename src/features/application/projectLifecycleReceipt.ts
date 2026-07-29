import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import type {
  LifecycleMutationKind,
  LifecycleMutationResultDto,
  ProjectRecordRow,
} from '@/shared/types/dto/project';

export interface PreparedProjectLifecycleTransition {
  readonly projectInstanceId: string;
  readonly publicationRevision: number;
  commit(): void;
}

export interface ProjectLifecycleReceiptDependencies {
  prepareProjectTransition(): Promise<PreparedProjectLifecycleTransition | null>;
  refreshRegistry(): Promise<ProjectRecordRow[]>;
  clearProject(): void;
  markProjectStale(): void;
}

export type ProjectLifecycleReceiptSource = 'direct' | 'event';
export type ProjectLifecycleReceiptStatus = 'applied' | 'duplicate' | 'stale';

export interface ProjectLifecycleReceiptSettlement {
  status: ProjectLifecycleReceiptStatus;
  result: LifecycleMutationResultDto;
  registryProjects?: ProjectRecordRow[];
}

export interface PendingProjectLifecycleOperation {
  readonly operationId: string;
  readonly kind: LifecycleMutationKind;
  readonly projectInstanceId: string | null;
  readonly coordinatorEpoch: number;
  isCurrent(): boolean;
}

interface ProjectLifecycleRegistryEntry extends PendingProjectLifecycleOperation {
  readonly expectsActiveProject: boolean;
  readonly registrationGeneration: number;
  state: 'pending' | 'processing' | 'complete' | 'directLost';
  fingerprint?: string;
  processing?: Promise<ProjectLifecycleReceiptSettlement>;
  settled?: ProjectLifecycleReceiptSettlement;
  notificationClaimed: boolean;
  expiresAt?: number;
  transition?: { projectInstanceId: string | null; coordinatorEpoch: number };
}

const MAX_PENDING_LIFECYCLE_OPERATIONS = 128;
export const PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS = 30_000;
const pendingOperations = new Map<string, ProjectLifecycleRegistryEntry>();
const registrationGenerations = new Map<string, number>();
let lifecycleClock = () => Date.now();

export class ProjectLifecycleProtocolError extends Error {
  readonly code = 'project_lifecycle_protocol_error';

  constructor(
    message: string,
    readonly zeroEffects = false,
  ) {
    super(message);
    this.name = 'ProjectLifecycleProtocolError';
  }
}

function stableValue(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(stableValue);
  if (!value || typeof value !== 'object') return value;
  return Object.fromEntries(
    Object.entries(value as Record<string, unknown>)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nested]) => [key, stableValue(nested)]),
  );
}

function fingerprintReceipt(result: LifecycleMutationResultDto): string {
  return JSON.stringify(stableValue(result));
}

function ownsLifecycle(projectInstanceId: string | null, coordinatorEpoch: number): boolean {
  return projectPublicationCoordinator.ownsApplicationLifecycle(
    projectInstanceId,
    coordinatorEpoch,
  );
}

function entryIsCurrent(entry: ProjectLifecycleRegistryEntry): boolean {
  if (entry.transition) {
    return ownsLifecycle(
      entry.transition.projectInstanceId,
      entry.transition.coordinatorEpoch,
    );
  }
  return ownsLifecycle(entry.projectInstanceId, entry.coordinatorEpoch);
}

function sweepLifecycleRegistry(): void {
  const now = lifecycleClock();
  for (const [operationId, entry] of pendingOperations) {
    if (!entryIsCurrent(entry) || (entry.expiresAt !== undefined && entry.expiresAt <= now)) {
      pendingOperations.delete(operationId);
    }
  }
}

function validateReceipt(
  entry: ProjectLifecycleRegistryEntry,
  result: LifecycleMutationResultDto,
): void {
  const isInactiveRegistryCleanup = entry.kind === 'delete'
    && !entry.expectsActiveProject
    && result.kind === 'registryCleanup';
  const isActiveRegistryCleanupRejection = entry.kind === 'delete'
    && entry.expectsActiveProject
    && result.kind === 'registryCleanup'
    && result.oldProjectInstanceId === null
    && result.newProjectInstanceId === null
    && result.phase === 'registryCommitted'
    && result.outcome === 'registryFailed'
    && result.record !== null
    && result.path === null
    && result.recovery?.required === true
    && result.recovery.action === 'cleanupRegistry'
    && result.recovery.path === null
    && result.recovery.identity === null
    && !result.invalidation.project
    && result.invalidation.registry;
  if (result.kind !== entry.kind
    && !isInactiveRegistryCleanup
    && !isActiveRegistryCleanupRejection) {
    throw new ProjectLifecycleProtocolError(
      `operation '${result.operationId}' changed lifecycle kind`,
      true,
    );
  }
  const expectedOldIdentity = isActiveRegistryCleanupRejection
    ? null
    : entry.expectsActiveProject
      ? entry.projectInstanceId
      : null;
  if (result.oldProjectInstanceId !== expectedOldIdentity) {
    throw new ProjectLifecycleProtocolError(
      `operation '${result.operationId}' changed initiating project identity`,
      true,
    );
  }
  const fingerprint = fingerprintReceipt(result);
  if (entry.fingerprint && entry.fingerprint !== fingerprint) {
    throw new ProjectLifecycleProtocolError(
      `operation '${result.operationId}' produced conflicting lifecycle receipts`,
      true,
    );
  }
  entry.fingerprint = fingerprint;
}

function captureOwnedTransition(entry: ProjectLifecycleRegistryEntry): void {
  const lifecycle = projectPublicationCoordinator.captureApplicationLifecycle();
  entry.transition = {
    projectInstanceId: lifecycle.projectInstanceId,
    coordinatorEpoch: lifecycle.epoch,
  };
}

function assertEntryCurrent(entry: ProjectLifecycleRegistryEntry): void {
  if (!entryIsCurrent(entry)) {
    throw new ProjectLifecycleProtocolError(
      `operation '${entry.operationId}' lost its lifecycle owner during settlement`,
    );
  }
}

async function rehydrateAndTransition(
  entry: ProjectLifecycleRegistryEntry,
  result: LifecycleMutationResultDto,
  dependencies: ProjectLifecycleReceiptDependencies,
): Promise<void> {
  assertEntryCurrent(entry);
  if (result.newProjectInstanceId) {
    projectPublicationCoordinator.startProject(result.newProjectInstanceId, 0);
    captureOwnedTransition(entry);
  }
  const prepared = await dependencies.prepareProjectTransition();
  assertEntryCurrent(entry);
  if (!prepared) {
    dependencies.markProjectStale();
    throw new Error('Project lifecycle hydration preparation returned no project');
  }
  if (result.outcome === 'committed'
    && result.newProjectInstanceId
    && prepared.projectInstanceId !== result.newProjectInstanceId) {
    dependencies.markProjectStale();
    throw new ProjectLifecycleProtocolError(
      `operation '${entry.operationId}' prepared an unexpected project identity`,
    );
  }
  prepared.commit();
  if (useProjectIOStore.getState().projectInstanceId !== prepared.projectInstanceId) {
    throw new ProjectLifecycleProtocolError(
      `operation '${entry.operationId}' committed an unexpected project store identity`,
    );
  }
  captureOwnedTransition(entry);
  if (!entryIsCurrent(entry)) {
    throw new ProjectLifecycleProtocolError(
      `operation '${entry.operationId}' did not own its committed project transition`,
    );
  }
}

async function processReceipt(
  entry: ProjectLifecycleRegistryEntry,
  result: LifecycleMutationResultDto,
  dependencies: ProjectLifecycleReceiptDependencies,
): Promise<ProjectLifecycleReceiptSettlement> {
  if (!entryIsCurrent(entry)) return { status: 'stale', result };

  if (result.kind === 'delete' && result.invalidation.project) {
    assertEntryCurrent(entry);
    dependencies.clearProject();
    captureOwnedTransition(entry);
  } else if (result.kind === 'saveAs'
    && result.invalidation.project
    && (result.outcome === 'committed' || result.outcome === 'activationFailed')) {
    await rehydrateAndTransition(entry, result, dependencies);
  }

  let registryProjects: ProjectRecordRow[] | undefined;
  if (result.invalidation.registry) {
    assertEntryCurrent(entry);
    registryProjects = await dependencies.refreshRegistry();
    assertEntryCurrent(entry);
  }

  return { status: 'applied', result, registryProjects };
}

export function registerPendingProjectLifecycleOperation(options: {
  kind: LifecycleMutationKind;
  operationId?: string;
  expectsActiveProject?: boolean;
}): PendingProjectLifecycleOperation {
  sweepLifecycleRegistry();
  const operationId = options.operationId ?? crypto.randomUUID();
  if (pendingOperations.has(operationId)) {
    throw new ProjectLifecycleProtocolError(
      `project lifecycle operation '${operationId}' is already registered`,
    );
  }
  if (pendingOperations.size >= MAX_PENDING_LIFECYCLE_OPERATIONS) {
    const completed = [...pendingOperations.entries()]
      .find(([, entry]) => entry.state === 'complete' && entry.notificationClaimed);
    if (completed) pendingOperations.delete(completed[0]);
  }
  if (pendingOperations.size >= MAX_PENDING_LIFECYCLE_OPERATIONS) {
    throw new ProjectLifecycleProtocolError('Too many pending project lifecycle operations');
  }
  const lifecycle = projectPublicationCoordinator.captureApplicationLifecycle();
  const storeProjectInstanceId = useProjectIOStore.getState().projectInstanceId;
  if (storeProjectInstanceId !== lifecycle.projectInstanceId) {
    throw new ProjectLifecycleProtocolError(
      'Project store identity does not match the application lifecycle owner',
    );
  }
  const expectsActiveProject = options.expectsActiveProject
    ?? (options.kind !== 'create' && lifecycle.projectInstanceId !== null);
  const registrationGeneration = (registrationGenerations.get(operationId) ?? 0) + 1;
  registrationGenerations.set(operationId, registrationGeneration);
  const entry: ProjectLifecycleRegistryEntry = {
    operationId,
    kind: options.kind,
    registrationGeneration,
    projectInstanceId: lifecycle.projectInstanceId,
    coordinatorEpoch: lifecycle.epoch,
    expectsActiveProject,
    state: 'pending',
    notificationClaimed: false,
    isCurrent: () => entryIsCurrent(entry),
  };
  pendingOperations.set(operationId, entry);
  return entry;
}

export async function applyProjectLifecycleReceipt(
  result: LifecycleMutationResultDto,
  source: ProjectLifecycleReceiptSource,
  dependencies: ProjectLifecycleReceiptDependencies,
): Promise<ProjectLifecycleReceiptSettlement> {
  sweepLifecycleRegistry();
  const entry = pendingOperations.get(result.operationId);
  if (!entry) return { status: 'stale', result };
  if (source === 'event' && entry.registrationGeneration > 1 && !entry.fingerprint) {
    return { status: 'stale', result };
  }
  validateReceipt(entry, result);
  if (!entryIsCurrent(entry)) return { status: 'stale', result };
  if (entry.state === 'complete' && entry.settled) {
    return { ...entry.settled, status: 'duplicate' };
  }

  if (entry.state === 'processing' && entry.processing) {
    try {
      const settled = await entry.processing;
      return { ...settled, status: 'duplicate' };
    } catch (error) {
      if (source === 'event') throw error;
    }
    if (!entryIsCurrent(entry)) return { status: 'stale', result };
  }

  entry.state = 'processing';
  const processing = processReceipt(entry, result, dependencies);
  entry.processing = processing;
  try {
    const settlement = await processing;
    if (entry.processing !== processing) return { status: 'duplicate', result };
    entry.processing = undefined;
    entry.state = settlement.status === 'applied' ? 'complete' : 'pending';
    if (entry.state === 'complete') {
      entry.settled = settlement;
      entry.expiresAt = lifecycleClock() + PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS;
    }
    return settlement;
  } catch (error) {
    if (entry.processing === processing) {
      entry.processing = undefined;
      entry.state = 'pending';
    }
    throw error;
  }
}

export function claimProjectLifecycleInitiatorSettlement(
  operationId: string,
): ProjectLifecycleReceiptSettlement | undefined {
  sweepLifecycleRegistry();
  const entry = pendingOperations.get(operationId);
  if (!entry
    || entry.notificationClaimed
    || entry.state !== 'complete'
    || !entry.settled) {
    return undefined;
  }
  entry.notificationClaimed = true;
  entry.expiresAt = lifecycleClock();
  return entry.settled;
}

export function claimProjectLifecycleNotification(operationId: string): boolean {
  return claimProjectLifecycleInitiatorSettlement(operationId) !== undefined;
}

export async function recoverProjectLifecycleDirectFailure(
  operationId: string,
): Promise<ProjectLifecycleReceiptSettlement | undefined> {
  sweepLifecycleRegistry();
  let entry = pendingOperations.get(operationId);
  if (!entry) return undefined;
  if (entry.state === 'complete') return entry.settled;
  if (entry.state === 'processing' && entry.processing) {
    try {
      return await entry.processing;
    } catch {
      await Promise.resolve();
      entry = pendingOperations.get(operationId);
      if (!entry) return undefined;
    }
  }
  if (entry.state === 'pending') {
    entry.state = 'directLost';
    entry.expiresAt = lifecycleClock() + PROJECT_LIFECYCLE_SETTLEMENT_TTL_MS;
  }
  return undefined;
}

export function cancelPendingProjectLifecycleOperation(operationId: string): void {
  const entry = pendingOperations.get(operationId);
  if (entry && entry.state !== 'complete') pendingOperations.delete(operationId);
}

export function getProjectLifecycleRegistrySizeForTests(): number {
  return pendingOperations.size;
}

export function getProjectLifecycleOperationForTests(operationId: string): {
  state: ProjectLifecycleRegistryEntry['state'];
} | undefined {
  const entry = pendingOperations.get(operationId);
  return entry ? { state: entry.state } : undefined;
}

export function setProjectLifecycleClockForTests(clock: () => number): void {
  lifecycleClock = clock;
}

export function resetProjectLifecycleReceiptHandlerForTests(): void {
  pendingOperations.clear();
  registrationGenerations.clear();
  lifecycleClock = () => Date.now();
}
