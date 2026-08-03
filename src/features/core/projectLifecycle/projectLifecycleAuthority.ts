export interface ProjectIdentitySnapshot {
  readonly projectInstanceId: string;
  readonly epoch: number;
}

export interface ProjectLifecycleStateSnapshot {
  readonly projectInstanceId: string | null;
  readonly epoch: number;
}

export type ProjectLifecycleActivationResult = 'activated' | 'duplicate' | 'stale';

export class ProjectLifecycleError extends Error {
  readonly code = 'stale_project_lifecycle';

  constructor() {
    super('project lifecycle changed before publication settlement');
    this.name = 'ProjectLifecycleError';
  }
}

let activeProjectInstanceId: string | null = null;
let projectEpoch = 0;
let latestActivationRevision = 0;

function staleLifecycleError(): ProjectLifecycleError {
  return new ProjectLifecycleError();
}

export function startProjectLifecycle(projectInstanceId: string): void {
  projectEpoch += 1;
  activeProjectInstanceId = projectInstanceId;
}

export function clearProjectLifecycle(): void {
  projectEpoch += 1;
  activeProjectInstanceId = null;
}

export function acceptProjectLifecycleActivation(
  projectInstanceId: string,
  activationRevision: number,
): ProjectLifecycleActivationResult {
  if (activationRevision < latestActivationRevision) return 'stale';
  if (activationRevision === latestActivationRevision) {
    return activeProjectInstanceId === projectInstanceId ? 'duplicate' : 'stale';
  }
  latestActivationRevision = activationRevision;
  startProjectLifecycle(projectInstanceId);
  return 'activated';
}

export function captureProjectLifecycleState(): ProjectLifecycleStateSnapshot {
  return Object.freeze({
    projectInstanceId: activeProjectInstanceId,
    epoch: projectEpoch,
  });
}

export function isProjectLifecycleStateCurrent(
  snapshot: ProjectLifecycleStateSnapshot,
): boolean {
  return activeProjectInstanceId === snapshot.projectInstanceId
    && projectEpoch === snapshot.epoch;
}

export function captureProjectIdentity(): ProjectIdentitySnapshot {
  if (!activeProjectInstanceId) throw staleLifecycleError();
  return Object.freeze({
    projectInstanceId: activeProjectInstanceId,
    epoch: projectEpoch,
  });
}

export function isCurrentProjectIdentity(identity: ProjectIdentitySnapshot): boolean {
  return activeProjectInstanceId === identity.projectInstanceId
    && projectEpoch === identity.epoch;
}

export function assertCurrentProjectIdentity(identity: ProjectIdentitySnapshot): void {
  if (!isCurrentProjectIdentity(identity)) throw staleLifecycleError();
}
