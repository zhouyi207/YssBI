import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import type { ResourceMutationResultDto } from '@/shared/types/dto/editorMutation';
import { DatabaseService } from './databaseService';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const projectInstanceId = '00000000-0000-0000-0000-000000000601';
const operationId = '00000000-0000-0000-0000-000000000401';
const expectedRevision = 4;
const mutation = {
  operationId,
  projectInstanceId,
  publicationRevision: 1,
  moves: [],
  deltas: [],
  projectionReplacements: [],
  projectionStatus: { status: 'complete', expectedGraphPaths: [] },
  history: { canUndo: false, canRedo: false },
} satisfies ResourceMutationResultDto;

beforeEach(() => {
  vi.clearAllMocks();
});

describe('DatabaseService revisioned mutation contract', () => {
  it('passes caller project and operation identity for expected-absent imports and returns the aggregate', async () => {
    const engine = { csv: { path: 'C:/sales.csv', delimiter: ',', hasHeader: true } } as const;
    const aggregate = {
      data: { id: 'sales', name: 'Sales', rowCount: 1, columnCount: 1, columns: [] },
      mutation,
    };
    vi.mocked(invoke).mockResolvedValue(aggregate);

    await expect(DatabaseService.loadDatabase(projectInstanceId, operationId, engine))
      .resolves.toBe(aggregate);
    expect(invoke).toHaveBeenCalledWith('load_database', {
      projectInstanceId,
      operationId,
      engine,
    });
  });

  it.each([
    ['deleteDatabase', 'delete_database', [projectInstanceId, operationId, expectedRevision, 'sales'], {}],
    ['renameDatabase', 'rename_database', [projectInstanceId, operationId, expectedRevision, 'sales', 'Renamed'], { name: 'Renamed' }],
    ['editCell', 'edit_cell', [projectInstanceId, operationId, expectedRevision, 'sales', 2, 'amount', 9, 12], { row: 2, colName: 'amount', value: 9, rowId: 12 }],
    ['addRow', 'add_row', [projectInstanceId, operationId, expectedRevision, 'sales', 3], { index: 3 }],
    ['deleteRows', 'delete_rows', [projectInstanceId, operationId, expectedRevision, 'sales', [1], [11]], { indices: [1], rowIds: [11] }],
    ['addColumn', 'add_column', [projectInstanceId, operationId, expectedRevision, 'sales', 'tax', 'Float64'], { name: 'tax', dtype: 'Float64' }],
    ['deleteColumn', 'delete_column', [projectInstanceId, operationId, expectedRevision, 'sales', 'tax'], { name: 'tax' }],
    ['castColumn', 'cast_column', [projectInstanceId, operationId, expectedRevision, 'sales', 'amount', 'Int64', true], { colName: 'amount', newDtype: 'Int64', force: true }],
    ['renameColumn', 'rename_column', [projectInstanceId, operationId, expectedRevision, 'sales', 'old', 'next'], { oldName: 'old', newName: 'next' }],
    ['undoEdit', 'undo_edit', [projectInstanceId, operationId, expectedRevision, 'sales'], {}],
    ['redoEdit', 'redo_edit', [projectInstanceId, operationId, expectedRevision, 'sales'], {}],
    ['saveDatabaseChanges', 'save_database_changes', [projectInstanceId, operationId, expectedRevision, 'sales'], {}],
  ] as const)('passes exact revision authority through %s', async (method, command, args, extra) => {
    const aggregate = { data: method === 'deleteDatabase' || method === 'renameDatabase' ? null : { isModified: false }, mutation };
    vi.mocked(invoke).mockResolvedValue(aggregate);

    await expect((DatabaseService[method] as (...values: any[]) => Promise<unknown>)(...args))
      .resolves.toBe(aggregate);
    expect(invoke).toHaveBeenCalledWith(command, {
      projectInstanceId,
      operationId,
      expectedRevision,
      id: 'sales',
      ...extra,
    });
  });
});
