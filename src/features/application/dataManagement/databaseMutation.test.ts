import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { useDatabaseStore } from '@/features/core/dataStore/databaseStore';
import { executeDatabaseMutation } from './databaseMutation';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000602';

function aggregate(operationId: string) {
  return {
    data: 'done',
    mutation: {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [],
      projectionReplacements: [],
      projectionStatus: { status: 'complete' as const, expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: false },
    },
  };
}

describe('executeDatabaseMutation', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useDatabaseStore.setState({ databases: {}, revisions: { sales: 4 } });
  });

  it('passes one revisioned lifecycle snapshot to the command and settles its receipt', async () => {
    const command = vi.fn(async (authority) => aggregate(authority.operationId));

    await expect(executeDatabaseMutation('sales', command)).resolves.toBe('done');
    expect(command).toHaveBeenCalledWith({
      projectInstanceId,
      operationId: expect.any(String),
      expectedRevision: 4,
    });
  });

  it('rejects lifecycle replacement inside the authority reader before command or publication effects', async () => {
    const authority = useDatabaseStore.getState();
    const before = {
      databases: structuredClone(authority.databases),
      revisions: structuredClone(authority.revisions),
    };
    vi.spyOn(useDatabaseStore, 'getState').mockImplementationOnce(() => {
      projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
      return authority;
    });
    const command = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(executeDatabaseMutation('sales', command)).rejects.toMatchObject({
      code: 'stale_project_lifecycle',
    });

    expect(command).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
    expect(useDatabaseStore.getState()).toMatchObject(before);
  });

  it('rejects missing revision authority before command or publication effects', async () => {
    useDatabaseStore.setState({ databases: {}, revisions: {} });
    const command = vi.fn();
    const submit = vi.spyOn(projectPublicationCoordinator, 'submit');

    await expect(executeDatabaseMutation('sales', command)).rejects.toThrow(
      "Database 'sales' has no authoritative revision",
    );

    expect(command).not.toHaveBeenCalled();
    expect(submit).not.toHaveBeenCalled();
  });
});
