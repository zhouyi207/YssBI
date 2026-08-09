import { beforeEach, describe, expect, it, vi } from 'vitest';

const ipc = vi.hoisted(() => ({
  response: undefined as unknown,
  invoke: vi.fn(async () => ipc.response),
}));

vi.mock('@tauri-apps/api/core', () => ({
  Channel: class {},
  invoke: ipc.invoke,
}));

import { ProjectService } from './projectService';

function projectIndex(): Record<string, unknown> {
  return {
    projectInstanceId: '00000000-0000-0000-0000-000000000601',
    publicationRevision: 4,
    history: { canUndo: false, canRedo: false },
    projectName: 'Projection contract',
    exportTime: '2026-08-07T00:00:00Z',
    graphs: [{
      path: 'functions/forecast.yssbi-function',
      name: 'Forecast',
      type: 'function',
      revision: 11,
      functionRevision: 11,
      functionSignature: {
        parameters: [{ id: 'sales', name: 'Observed sales', type_name: 'DataSeries<Float64>' }],
        return_type: 'Array<String>',
      },
      functionEditorProjection: {
        functionRevision: 11,
        inputs: [{
          id: 'sales',
          name: 'Observed sales',
          dataType: { kind: 'DataSeries', inner: { kind: 'Float64' } },
        }],
        outputs: [{
          id: 'return',
          name: 'Array<String>',
          dataType: { kind: 'Array', inner: { kind: 'String' } },
        }],
      },
    }],
    worksheets: [],
    variables: [],
    databases: [],
  };
}

function functionRow(index: Record<string, unknown>): Record<string, unknown> {
  return (index.graphs as Array<Record<string, unknown>>)[0];
}

describe('ProjectService.getProjectIndex function editor projection parser', () => {
  beforeEach(() => {
    ipc.invoke.mockClear();
    ipc.response = projectIndex();
  });

  it('preserves the exact Rust-resolved output name and structured pin types', async () => {
    const index = await ProjectService.getProjectIndex(
      '00000000-0000-0000-0000-000000000601',
    );

    const functionRow = index.graphs[0];
    expect(functionRow.type).toBe('function');
    if (functionRow.type !== 'function') throw new Error('expected function row');
    expect(functionRow.functionEditorProjection).toEqual({
      functionRevision: 11,
      inputs: [{
        id: 'sales',
        name: 'Observed sales',
        dataType: { kind: 'DataSeries', inner: { kind: 'Float64' } },
      }],
      outputs: [{
        id: 'return',
        name: 'Array<String>',
        dataType: { kind: 'Array', inner: { kind: 'String' } },
      }],
    });
  });

  it('accepts opaque event and function paths containing spaces and Unicode', async () => {
    const index = projectIndex();
    const row = functionRow(index);
    row.path = 'functions/Sales Report 销售预测.yssbi-function';
    (index.graphs as unknown[]).unshift({
      path: 'events/每日 Sales Report.yssbi-event',
      name: 'Daily report',
      type: 'event',
      revision: 3,
    });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).resolves.toMatchObject({
      graphs: [
        { path: 'events/每日 Sales Report.yssbi-event', type: 'event' },
        { path: 'functions/Sales Report 销售预测.yssbi-function', type: 'function' },
      ],
    });
  });

  it('rejects project rows whose type disagrees with the path kind or suffix', async () => {
    for (const path of [
      'events/Wrong.yssbi-event',
      'functions/Wrong.yssbi-event',
      'functions/Wrong.txt',
    ]) {
      const index = projectIndex();
      functionRow(index).path = path;
      ipc.response = index;
      await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
        'Invalid project index response',
      );
    }
  });

  it('rejects a function editor projection missing inputs', async () => {
    const index = projectIndex();
    const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
    delete projection.inputs;
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
  });

  it('rejects malformed structured data types instead of falling back to Any', async () => {
    const index = projectIndex();
    const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
    const input = (projection.inputs as Array<Record<string, unknown>>)[0];
    input.dataType = { kind: 'DataSeries', inner: { kind: 'UnknownLegacyType' } };
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
  });

  it('rejects empty and whitespace-only Struct keys', async () => {
    for (const inner of ['', '   ']) {
      const index = projectIndex();
      const projection = functionRow(index).functionEditorProjection as Record<string, unknown>;
      const output = (projection.outputs as Array<Record<string, unknown>>)[0];
      output.dataType = { kind: 'Struct', inner };
      ipc.response = index;

      await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
        'Invalid project index response',
      );
    }
  });

  it('rejects a legacy project index containing the removed application version key', async () => {
    const index = projectIndex();
    index[['app', 'Version'].join('')] = '0.2.7';
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
  });

  it('requires every exact project-index key to be an own property', async () => {
    const index = projectIndex();
    const projectName = index.projectName;
    delete index.projectName;
    index.unknownProjectName = 'substitution';
    ipc.response = Object.assign(Object.create({ projectName }), index);

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
  });

  it('rejects unknown keys before evaluating graph and database values', async () => {
    const index = projectIndex();
    let graphsEvaluated = false;
    let databasesEvaluated = false;
    index.unknownLegacyKey = true;
    Object.defineProperties(index, {
      graphs: {
        enumerable: true,
        get: () => {
          graphsEvaluated = true;
          throw new Error('graphs evaluated');
        },
      },
      databases: {
        enumerable: true,
        get: () => {
          databasesEvaluated = true;
          throw new Error('databases evaluated');
        },
      },
    });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
    expect(graphsEvaluated).toBe(false);
    expect(databasesEvaluated).toBe(false);
  });

  it('rejects inherited required keys before evaluating invalid rows', async () => {
    const index = projectIndex();
    const projectName = index.projectName;
    let graphsEvaluated = false;
    let databasesEvaluated = false;
    delete index.projectName;
    index.unknownProjectName = 'substitution';
    Object.defineProperties(index, {
      graphs: {
        enumerable: true,
        get: () => {
          graphsEvaluated = true;
          return [null];
        },
      },
      databases: {
        enumerable: true,
        get: () => {
          databasesEvaluated = true;
          return [null];
        },
      },
    });
    Object.setPrototypeOf(index, { projectName });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
    expect(graphsEvaluated).toBe(false);
    expect(databasesEvaluated).toBe(false);
  });

  it('rejects a legacy function row containing only raw functionSignature', async () => {
    const index = projectIndex();
    const row = functionRow(index);
    delete row.functionEditorProjection;
    Object.assign(row, {
      functionRevision: 11,
      functionSignature: {
        parameters: [{ id: 'sales', name: 'Observed sales', type_name: 'DataSeries<Float64>' }],
        return_type: 'Array<String>',
      },
    });
    ipc.response = index;

    await expect(ProjectService.getProjectIndex('project-a')).rejects.toThrow(
      'Invalid project index response',
    );
  });
});
