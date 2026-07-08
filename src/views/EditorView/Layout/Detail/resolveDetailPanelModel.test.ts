import { describe, expect, it } from 'vitest';
import type { LogMessage } from '@/shared/types/ui';
import { resolveDetailPanelModel } from './resolveDetailPanelModel';

const logEntry = { level: 'info', message: 'hello' } as LogMessage;

const catalog = {
  variables: {
    'var-1': {
      id: 'var-1',
      name: 'X',
      dataType: { kind: 'Float64' as const },
      dataValue: { kind: 'Float64' as const, value: 1 },
      scope: { type: 'global' as const },
      description: '',
      tags: [],
    },
  },
  events: { 'evt-1': { id: 'evt-1', name: 'Main' } },
  functions: { 'fn-1': { id: 'fn-1', name: 'Add' } },
  dataframes: {
    'df-1': { id: 'df-1', name: 'Sales', rowCount: 10 },
  },
};

describe('resolveDetailPanelModel', () => {
  it('returns empty when target is null', () => {
    expect(
      resolveDetailPanelModel({
        target: null,
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toEqual({ kind: 'empty' });
  });

  it('resolves resource-backed panels from catalog snapshots', () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: 'variable', id: 'var-1' },
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toMatchObject({ kind: 'variable', id: 'var-1', variable: { name: 'X' } });

    expect(
      resolveDetailPanelModel({
        target: { kind: 'data', id: 'df-1' },
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toMatchObject({ kind: 'data', id: 'df-1', dataframe: { name: 'Sales' } });
  });

  it('merges function signature pins into function panel model', () => {
    const model = resolveDetailPanelModel({
      target: { kind: 'function', id: 'fn-1' },
      selectedLog: null,
      worksheetDocument: null,
      functionSignature: {
        functionInputs: [{ id: 'in-1', name: 'A', type: 'Float64' }],
        functionOutputs: [{ id: 'out-1', name: 'R', type: 'Float64' }],
      },
      ...catalog,
    });

    expect(model).toEqual({
      kind: 'function',
      id: 'fn-1',
      fn: {
        id: 'fn-1',
        name: 'Add',
        inputs: [{ id: 'in-1', name: 'A', type: 'Float64' }],
        outputs: [{ id: 'out-1', name: 'R', type: 'Float64' }],
      },
    });
  });

  it('returns empty when catalog entry is missing or log is not selected', () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: 'event', id: 'missing' },
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toEqual({ kind: 'empty' });

    expect(
      resolveDetailPanelModel({
        target: { kind: 'log' },
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toEqual({ kind: 'empty' });

    expect(
      resolveDetailPanelModel({
        target: { kind: 'log' },
        selectedLog: logEntry,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toEqual({ kind: 'log', log: logEntry });
  });
});
