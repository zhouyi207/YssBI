// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { useDatabaseStore } from '@/features/core/dataStore';
import { useEditorStore } from '@/features/core/editor';
import { useResourceStore } from '@/features/core/resource';
import { projectPublicationCoordinator } from '@/features/application/editorMutation/projectPublicationCoordinator';
import { DatabaseService } from '@/services/database/databaseService';
import { useDatabaseManagement } from './useDatabaseManagement';

vi.mock('@tauri-apps/plugin-dialog', () => ({ open: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function aggregate(afterName: string | null, operationId: string) {
  const before = {
    id: 'sales',
    engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
    schemaVersion: 1,
    required: false,
    name: 'Sales',
  };
  return {
    data: null,
    mutation: {
      operationId,
      projectInstanceId,
      publicationRevision: 1,
      moves: [],
      deltas: [{
        resource: { kind: 'database' as const, key: 'opaque database resource path' },
        fromRevision: 4,
        toRevision: 5,
        causedBy: operationId,
        payload: {
          kind: 'database' as const,
          patch: { before, after: afterName === null ? null : { ...before, name: afterName } },
        },
      }],
      projectionReplacements: [],
      projectionStatus: { status: 'complete' as const, expectedGraphPaths: [] },
      history: { canUndo: false, canRedo: false },
    },
  };
}

describe('useDatabaseManagement revision authority', () => {
  let root: Root;
  let host: HTMLDivElement;
  let actions: ReturnType<typeof useDatabaseManagement>;

  beforeEach(() => {
    vi.restoreAllMocks();
    vi.clearAllMocks();
    projectPublicationCoordinator.cancelProject();
    projectPublicationCoordinator.startProject(projectInstanceId, 0);
    useEditorStore.getState().clearDetailFocus();
    useResourceStore.getState().clear();
    useDatabaseStore.setState({
      databases: {
        sales: {
          id: 'sales',
          name: 'Sales',
          resourcePath: 'opaque database resource path',
          engine: { duckDb: { path: 'database/project.duckdb', table: 'sales' } },
          schemaVersion: 1,
          required: false,
        },
      },
      revisions: { sales: 4 },
    });
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    function Harness() {
      actions = useDatabaseManagement();
      return null;
    }
    act(() => root.render(<Harness />));
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('passes exact database revision and submits the canonical aggregate mutation', async () => {
    vi.spyOn(DatabaseService, 'renameDatabase').mockImplementation(
      async (_project, operation) => aggregate('Renamed', operation),
    );

    await act(async () => actions.renameDataFrame('sales', 'Renamed'));

    expect(DatabaseService.renameDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      4,
      'sales',
      'Renamed',
    );
    expect(useDatabaseStore.getState().revisions.sales).toBe(5);
    expect(useDatabaseStore.getState().databases.sales?.name).toBe('Renamed');
  });

  it('does not perform an independent delete outside canonical publication application', async () => {
    vi.spyOn(DatabaseService, 'deleteDatabase').mockImplementation(
      async (_project, operation) => aggregate(null, operation),
    );

    await act(async () => actions.deleteDataFrame('sales'));

    expect(DatabaseService.deleteDatabase).toHaveBeenCalledWith(
      projectInstanceId,
      expect.any(String),
      4,
      'sales',
    );
    expect(useDatabaseStore.getState().databases.sales).toBeUndefined();
    expect(useDatabaseStore.getState().revisions.sales).toBeUndefined();
  });
});
