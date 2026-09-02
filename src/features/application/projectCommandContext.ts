import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";
import {
  assertCurrentProjectIdentity,
  captureProjectIdentity,
  isCurrentProjectIdentity,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";

export interface ProjectCommandContext {
  projectInstanceId: string;
  projectEpoch: number;
  publicationRevision: number;
  operationId: string;
  operationPendingKey: string;
  isCurrent(): boolean;
  assertCurrent(): void;
}

export interface RevisionedProjectCommandSnapshot<T> {
  readonly context: ProjectCommandContext;
  readonly authority: T;
}

export function captureProjectCommandContext(
  requestedOperationId: string = crypto.randomUUID(),
): ProjectCommandContext {
  const identity = captureProjectIdentity();
  const publicationRevision = projectPublicationCoordinator.capturePublicationRevision();
  assertCurrentProjectIdentity(identity);
  const operationId = requestedOperationId;
  const isCurrent = () => isCurrentProjectIdentity(identity);
  return {
    projectInstanceId: identity.projectInstanceId,
    projectEpoch: identity.epoch,
    publicationRevision,
    operationId,
    operationPendingKey: `${identity.projectInstanceId}:${operationId}`,
    isCurrent,
    assertCurrent: () => assertCurrentProjectIdentity(identity),
  };
}

export function captureRevisionedProjectCommandSnapshot<T>(
  readAuthority: () => T,
): RevisionedProjectCommandSnapshot<T> {
  const context = captureProjectCommandContext();
  const authority = readAuthority();
  context.assertCurrent();
  return Object.freeze({ context, authority });
}
