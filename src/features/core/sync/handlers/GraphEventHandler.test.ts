import { beforeEach, describe, expect, it } from 'vitest';
import { createDataSignaturePin } from '@/shared/types/domain/functionSignaturePin';
import { useGraphMetaStore } from '@/features/core/dataStore';
import { buildGraphResourceMeta, useResourceStore } from '@/features/core/resource';
import { FunctionCreatedHandler, FunctionUpdatedHandler } from './GraphEventHandler';
import {
  markGraphRefreshEcho,
  resolveGraphRefreshEcho,
} from '@/features/application/graphDocument/graphRefreshEchoGuard';

describe('Graph event handlers', () => {
  beforeEach(() => {
    useGraphMetaStore.setState({ graphs: {} });
    useResourceStore.getState().clear();
  });

  it('syncs function signature metadata from FunctionCreated events', () => {
    new FunctionCreatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        path: 'functions/Compute.yssbi-function',
        name: 'Compute',
        type: 'function',
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
        nodes: [],
        pins: [],
        connections: { connections: [] },
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      }),
    );
  });

  it('syncs function signature metadata from FunctionUpdated events', () => {
    useResourceStore.getState().upsertResource(
      buildGraphResourceMeta('function', 'functions/Compute.yssbi-function', 'Compute', { loaded: true }),
    );

    new FunctionUpdatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        path: 'functions/Compute.yssbi-function',
        name: 'Compute',
        type: 'function',
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
        functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
      }),
    );
  });

  it('does not create function metadata from partial FunctionUpdated events without resource metadata', () => {
    new FunctionUpdatedHandler().handle({
      path: 'functions/Compute.yssbi-function',
      data: {
        functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
      },
    });

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toBeUndefined();
  });

  it('skips FunctionUpdated while invoke graph refresh echo guard is active', () => {
    useResourceStore.getState().upsertResource(
      buildGraphResourceMeta('function', 'functions/Compute.yssbi-function', 'Compute', { loaded: true }),
    );
    useGraphMetaStore.getState().addGraph({
      path: 'functions/Compute.yssbi-function',
      name: 'Compute',
      type: 'function',
      functionInputs: [],
      functionOutputs: [],
    });

    markGraphRefreshEcho(['functions/Compute.yssbi-function']);
    try {
      new FunctionUpdatedHandler().handle({
        path: 'functions/Compute.yssbi-function',
        data: {
          functionInputs: [createDataSignaturePin('input-1', 'Value', { kind: 'Int64' })],
          functionOutputs: [createDataSignaturePin('output-1', 'Result', { kind: 'Float64' })],
        },
      });
    } finally {
      resolveGraphRefreshEcho(['functions/Compute.yssbi-function']);
    }

    expect(useGraphMetaStore.getState().graphs['functions/Compute.yssbi-function']).toEqual(
      expect.objectContaining({
        functionInputs: [],
        functionOutputs: [],
      }),
    );
  });
});
