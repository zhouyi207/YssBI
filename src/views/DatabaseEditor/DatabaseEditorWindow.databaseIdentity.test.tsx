import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { DatabaseService } from '@/services/database/databaseService';
import type { LoadDatabaseResult } from '@/shared/types/dto/database';
import { hydrateDatabaseEditorMetadata } from '@/features/application/dataManagement/databaseRecords';
import { databasePublication } from '@/features/core/database/publication';

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

describe('database editor metadata lifecycle ownership', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
  });

  it('does not hydrate replacement state from an old metadata completion', async () => {
    const request = deferred<LoadDatabaseResult>();
    vi.spyOn(DatabaseService, 'getDatabaseMeta').mockReturnValue(request.promise);
    const isCancelled = vi.fn(() => false);
    const updateDatabase = vi.spyOn(databasePublication, 'updateDatabase');

    const completion = hydrateDatabaseEditorMetadata('sales', isCancelled);
    expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledWith(projectInstanceId, 'sales');
    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    request.resolve(meta);
    await completion;

    expect(isCancelled).toHaveBeenCalledOnce();
    expect(updateDatabase).not.toHaveBeenCalled();
  });
});
