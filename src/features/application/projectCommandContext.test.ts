import { beforeEach, describe, expect, it } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { captureRevisionedProjectCommandSnapshot } from './projectCommandContext';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000602';

describe('captureRevisionedProjectCommandSnapshot', () => {
  beforeEach(() => {
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 3);
  });

  it('returns authority with the lifecycle captured before the synchronous read', () => {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => ({ revision: 7 }));

    expect(snapshot.authority).toEqual({ revision: 7 });
    expect(snapshot.context).toMatchObject({
      projectInstanceId,
      publicationRevision: 3,
    });
    expect(snapshot.context.isCurrent()).toBe(true);
  });

  it('rejects the snapshot when the authority reader replaces the project lifecycle', () => {
    expect(() => captureRevisionedProjectCommandSnapshot(() => {
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      return { revision: 7 };
    })).toThrow(expect.objectContaining({ code: 'stale_project_lifecycle' }));
  });
});
