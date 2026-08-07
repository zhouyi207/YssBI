import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { DatabaseService } from '@/services/database/databaseService';
import type { LoadDatabaseResult } from '@/shared/types/dto/database';
import { hydrateBayesDatabaseMetadata } from './BayesView';

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000602';

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((settle) => { resolve = settle; });
  return { promise, resolve };
}

const meta: LoadDatabaseResult = {
  id: 'sales', name: 'Old sales', columns: [{ name: 'amount', type: 'Int64' }],
  rowCount: 1, columnCount: 1,
};

describe('Bayes database metadata lifecycle ownership', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
  });

  it('does not require project identity when no metadata refresh is needed', async () => {
    projectPublicationCoordinator.cancelProject();
    const updateDatabase = vi.fn();
    const getDatabaseMeta = vi.spyOn(DatabaseService, 'getDatabaseMeta');

    await expect(hydrateBayesDatabaseMetadata({}, updateDatabase)).resolves.toBeUndefined();

    expect(getDatabaseMeta).not.toHaveBeenCalled();
    expect(updateDatabase).not.toHaveBeenCalled();
  });

  it('does not hydrate replacement state from an old metadata completion', async () => {
    const request = deferred<LoadDatabaseResult>();
    vi.spyOn(DatabaseService, 'getDatabaseMeta').mockReturnValue(request.promise);
    const updateDatabase = vi.fn();

    const completion = hydrateBayesDatabaseMetadata({ sales: { id: 'sales', name: 'Sales' } }, updateDatabase);
    expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledWith(projectInstanceId, 'sales');
    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    request.resolve(meta);
    await completion;

    expect(updateDatabase).not.toHaveBeenCalled();
  });
});
