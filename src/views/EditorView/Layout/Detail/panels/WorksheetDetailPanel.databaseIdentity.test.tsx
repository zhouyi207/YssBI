// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot } from 'react-dom/client';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { DatabaseService } from '@/services/database/databaseService';
import type { LoadDatabaseResult } from '@/shared/types/dto/database';
import { hydrateWorksheetDatabaseMetadata, WorksheetDetailPanel } from './WorksheetDetailPanel';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@/features/application/editor', () => ({
  useEditorSessionResources: () => ({ dataframes: {} }),
}));
vi.mock('@/shared/ui/OverlayScrollbar', () => ({
  OverlayScrollbar: ({ children }: { children: React.ReactNode }) => <div>{children}</div>,
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

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

describe('worksheet detail metadata lifecycle ownership', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
  });

  it('commits the separately supplied Rust name through rename', async () => {
    const host = document.createElement('div');
    document.body.appendChild(host);
    const root = createRoot(host);
    const onRename = vi.fn();

    act(() => root.render(
      <WorksheetDetailPanel
        worksheetPath="worksheets/Path Does Not Reveal Label.yssbi-worksheet"
        name="Rust supplied label"
        document={{
          schemaVersion: 3,
          revision: 7,
          databaseId: '',
          chartType: 'scatter',
          encodings: {},
        }}
        onRename={onRename}
      />,
    ));
    const nameInput = host.querySelector('input[type="text"]') as HTMLInputElement;
    expect(nameInput.value).toBe('Rust supplied label');

    act(() => {
      const setValue = Object.getOwnPropertyDescriptor(
        HTMLInputElement.prototype,
        'value',
      )?.set;
      setValue?.call(nameInput, 'Renamed by user');
      nameInput.dispatchEvent(new Event('input', { bubbles: true }));
    });
    act(() => {
      nameInput.dispatchEvent(new FocusEvent('focusout', { bubbles: true }));
    });

    expect(onRename).toHaveBeenCalledWith('Renamed by user');
    act(() => root.unmount());
    host.remove();
  });

  it('does not hydrate replacement state from an old metadata completion', async () => {
    const request = deferred<LoadDatabaseResult>();
    vi.spyOn(DatabaseService, 'getDatabaseMeta').mockReturnValue(request.promise);
    const updateDatabase = vi.fn();

    const completion = hydrateWorksheetDatabaseMetadata('sales', updateDatabase);
    expect(DatabaseService.getDatabaseMeta).toHaveBeenCalledWith(projectInstanceId, 'sales');
    projectPublicationCoordinator.startProject(replacementProjectInstanceId, 0);
    request.resolve(meta);
    await completion;

    expect(updateDatabase).not.toHaveBeenCalled();
  });
});
