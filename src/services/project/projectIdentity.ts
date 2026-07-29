import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';

export interface ProjectIdentitySnapshot {
  projectInstanceId: string;
  epoch: number;
}

export interface ProjectCommandIdentitySnapshot extends ProjectIdentitySnapshot {
  publicationRevision: number;
}

export function captureProjectCommandIdentity(): ProjectCommandIdentitySnapshot {
  return projectPublicationCoordinator.captureCommandLifecycle();
}

export function captureProjectIdentity(): ProjectIdentitySnapshot {
  const { projectInstanceId, epoch } = captureProjectCommandIdentity();
  return { projectInstanceId, epoch };
}

export function isCurrentProjectIdentity(identity: ProjectIdentitySnapshot): boolean {
  return projectPublicationCoordinator.ownsCommandLifecycle(
    identity.projectInstanceId,
    identity.epoch,
  );
}

export function assertCurrentProjectIdentity(identity: ProjectIdentitySnapshot): void {
  projectPublicationCoordinator.assertCommandLifecycle(
    identity.projectInstanceId,
    identity.epoch,
  );
}
