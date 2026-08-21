import { describe, expect, it } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import type { DiagnosticRecordDto } from '@/shared/types/dto/diagnostics';
import { resolveDetailPanelModel } from './resolveDetailPanelModel';

const logEntry = {
  streamId: 'stream-1',
  sequence: 1,
  timestamp: '2026-08-16T10:11:12.000Z',
  level: 'info',
  origin: 'frontend',
  domain: 'application',
  target: 'test',
  message: 'hello',
  fields: {},
} satisfies DiagnosticRecordDto;

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
  functions: { 'fn-1': { id: 'fn-1', name: 'Add', functionInputs: [], functionOutputs: [] } },
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

  it('preserves the focused graph path for node detail selection', () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: 'node', id: 'shared-node', graphPath: 'functions/second' },
        selectedLog: null,
        worksheetDocument: null,
        ...catalog,
      }),
    ).toEqual({
      kind: 'node',
      nodeId: 'shared-node',
      graphPath: 'functions/second',
    });
  });

  it('merges function signature pins into function panel model', () => {
    const model = resolveDetailPanelModel({
      target: { kind: 'function', path: 'fn-1' },
      selectedLog: null,
      worksheetDocument: null,
      ...catalog,
      functions: {
        'fn-1': {
          id: 'fn-1',
          name: 'Add',
          functionInputs: [createDataSignaturePin('in-1', 'A', { kind: 'Float64' })],
          functionOutputs: [createDataSignaturePin('out-1', 'R', { kind: 'Float64' })],
        },
      },
    });

    expect(model).toEqual({
      kind: 'function',
      path: 'fn-1',
      fn: {
        name: 'Add',
        inputs: [createDataSignaturePin('in-1', 'A', { kind: 'Float64' })],
        outputs: [createDataSignaturePin('out-1', 'R', { kind: 'Float64' })],
      },
    });
  });

  it('returns empty when catalog entry is missing or log is not selected', () => {
    expect(
      resolveDetailPanelModel({
        target: { kind: 'event', path: 'missing' },
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
