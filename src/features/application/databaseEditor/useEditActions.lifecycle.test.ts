// @vitest-environment happy-dom
import { act, createElement } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { save } from '@tauri-apps/plugin-dialog';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { DatabaseService } from '@/services/database/databaseService';
import { logger } from '@/utils/appLogger';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditActions } from './useEditActions';

vi.mock('@tauri-apps/plugin-dialog', () => ({ save: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const replacementProjectInstanceId = '00000000-0000-0000-0000-000000000602';
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((settle, fail) => {
    resolve = settle;
    reject = fail;
  });
  return { promise, resolve, reject };
}

describe('useEditActions project lifecycle ownership', () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions!: ReturnType<typeof useEditActions>;
  let reloadAllData: ReturnType<typeof vi.fn<() => Promise<void>>>;

  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    vi.mocked(save).mockResolvedValue('C:/sales.csv');
    useDatabaseStore.setState({
      databases: { sales: { id: 'sales', name: 'Sales' } },
      revisions: { sales: 1 },
    });
    reloadAllData = vi.fn(async () => undefined);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useEditActions({
        selectedDfId: 'sales', columns: [], loadedRows: [], loadedRowIds: [], rowOffset: 0,
        reloadAllData,
      });
      return null;
    }
    act(() => root.render(createElement(Harness)));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('does not report a delayed export completion to a replacement project', async () => {
    const request = deferred<void>();
    vi.spyOn(DatabaseService, 'exportDatabase').mockReturnValue(request.promise);
    let completion!: Promise<void>;
    act(() => { completion = actions.handleExport(); });
    await vi.waitFor(() => expect(DatabaseService.exportDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      'sales',
      'C:/sales.csv',
      'csv',
    ));

    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    useDatabaseStore.setState({
      databases: { sales: { id: 'sales', name: 'Replacement' } },
      revisions: { sales: 0 },
    });
    const replacementStoreSnapshot = useDatabaseStore.getState();
    request.resolve();
    await act(async () => completion);

    expect(reloadAllData).not.toHaveBeenCalled();
    expect(useDatabaseStore.getState()).toBe(replacementStoreSnapshot);
  });

  it('suppresses a delayed export rejection after replacement with zero effects', async () => {
    const request = deferred<void>();
    vi.spyOn(DatabaseService, 'exportDatabase').mockReturnValue(request.promise);
    const log = vi.spyOn(logger.data, 'error');

    const completion = actions.handleExport();
    await vi.waitFor(() => expect(DatabaseService.exportDatabase).toHaveBeenCalled());
    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    useDatabaseStore.setState({
      databases: { sales: { id: 'sales', name: 'Replacement' } },
      revisions: { sales: 0 },
    });
    const replacementStoreSnapshot = useDatabaseStore.getState();
    request.reject(new Error('old export failed'));
    await act(async () => completion);

    expect(log).not.toHaveBeenCalled();
    expect(reloadAllData).not.toHaveBeenCalled();
    expect(useDatabaseStore.getState()).toBe(replacementStoreSnapshot);
  });
});
