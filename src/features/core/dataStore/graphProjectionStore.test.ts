import { readFileSync } from 'node:fs';
import { beforeEach, describe, expect, expectTypeOf, it, vi } from 'vitest';
import type {
  EditorGraphProjectionDto,
  DiagnosticDto,
  PortAddressDto,
  ProjectionBasisDto,
} from '@/shared/types/dto/editorProjection';
import { portAddressKey } from '@/features/domain/editorProjection';
import {
  getGraphDiagnostics,
  getGraphProjectionBasis,
  getGraphRequestGeneration,
  getGraphSourceRevision,
  hasGraphBlockingDiagnostics,
  type GraphEntityBucket,
} from './graphEntityAccess';
import { useGraphDataStore } from './graphDataStore';
import { toUiNode } from './nodeView';



const output: PortAddressDto = {
  kind: 'declared',
  nodeId: 'shared-node',
  portKey: 'output',
};
const input: PortAddressDto = {
  kind: 'instance',
  nodeId: 'shared-node',
  templateKey: 'input',
  instanceId: 'input-1',
};

function projection(
  graphPath = 'functions/main',
  sourceRevision = 4,
  title = 'Localized title',
): EditorGraphProjectionDto {
  const graphOutput = { ...output, nodeId: 'shared-node' };
  const graphInput = { ...input, nodeId: 'shared-node' };
  return {
    basis: {
      graphPath,
      graphRevision: sourceRevision,
      registryFingerprint: [1, 2, 3],
      resourceVersions: {},
    },
    graphPath,
    sourceRevision,
    nodes: [
      {
        graphPath,
        sourceRevision,
        nodeId: 'shared-node',
        nodeTypeId: 'unknown.projected-node',
        position: { x: 10, y: 20 },
        display: {
          title,
          description: 'Projected description',
          userLabel: null,
          iconId: 'projected-icon',
          styleId: 'projected-style',
        },
        ports: [
          {
            address: graphOutput,
            templateKey: 'output',
            display: { label: 'Output', instanceLabel: null },
            direction: 'output',
            kind: 'data',
            instanceKind: 'declared',
            orphan: false,
            canRemove: false,
            connections: {
              current: 1,
              maximum: null,
              ordered: false,
              canConnect: true,
            },
            input: null,
            resolvedType: { display: 'Number', resolved: true },
            resolvedSchema: null,
            status: 'resolved',
          },
          {
            address: graphInput,
            templateKey: 'input',
            display: { label: 'Input', instanceLabel: 'Input 1' },
            direction: 'input',
            kind: 'data',
            instanceKind: 'userCreated',
            orphan: false,
            canRemove: true,
            connections: {
              current: 1,
              maximum: 1,
              ordered: false,
              canConnect: false,
            },
            input: {
              literalOverride: 2,
              protocolDefault: 1,
              effective: 'connections',
            },
            resolvedType: { display: 'Number', resolved: true },
            resolvedSchema: null,
            status: 'resolved',
          },
        ],
        parameterEditors: [
          {
            key: 'factor',
            display: { title: 'Factor', description: null },
            editor: 'number',
            multiline: false,
            value: 2,
            configuration: null,
          },
        ],
        capabilities: {
          managed: false,
          canCopy: true,
          canDelete: true,
          canEditLabel: true,
          canEditParameters: true,
          hasDynamicPorts: true,
          supportsInlineLiterals: true,
        },
        diagnostics: [],
      },
    ],
    connections: [
      {
        connectionId: 'connection-1',
        output: graphOutput,
        input: graphInput,
        order: null,
      },
    ],
    diagnostics: [
      {
        code: 'graph.info',
        message: 'Projected diagnostic',
        severity: 'information',
        blocking: false,
        location: { kind: 'graph' },
        related: [],
      },
    ],
    hasBlockingDiagnostics: false,
  };
}

describe('graphDataStore projection replacement', () => {
  it('requires projection metadata on every graph bucket', () => {
    expectTypeOf<GraphEntityBucket>().toHaveProperty('basis').toEqualTypeOf<ProjectionBasisDto>();
    expectTypeOf<GraphEntityBucket>().toHaveProperty('sourceRevision').toEqualTypeOf<number>();
    expectTypeOf<GraphEntityBucket>().toHaveProperty('requestGeneration').toEqualTypeOf<number>();
    expectTypeOf<GraphEntityBucket>().toHaveProperty('diagnostics').toEqualTypeOf<DiagnosticDto[]>();
    expectTypeOf<GraphEntityBucket>()
      .toHaveProperty('hasBlockingDiagnostics')
      .toEqualTypeOf<boolean>();
    expect(getGraphProjectionBasis({ graphEntities: {} }, 'missing')).toBeUndefined();
    expect(getGraphSourceRevision({ graphEntities: {} }, 'missing')).toBeUndefined();
    expect(getGraphRequestGeneration({ graphEntities: {} }, 'missing')).toBeUndefined();
    expect(getGraphDiagnostics({ graphEntities: {} }, 'missing')).toBeUndefined();
    expect(hasGraphBlockingDiagnostics({ graphEntities: {} }, 'missing')).toBeUndefined();
  });

  it('keeps the canvas projection consumer independent from registry metadata', () => {
    const source = readFileSync(new URL('./nodeView.ts', import.meta.url), 'utf8');
    expect(source).not.toContain('resolveNodeViewMeta');
  });

  it('keeps projected canvas nodes independent from registry metadata', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection(), 1);

    const bucket = useGraphDataStore.getState().graphEntities['functions/main'];
    const canvasNode = toUiNode(bucket.nodes['shared-node'], {
      pins: bucket.nodePins['shared-node'].map((key) => ({
        pin: bucket.pins[key],
        connectionIds: bucket.pinConnections[key],
      })),
    });

    expect(canvasNode.position).toEqual({ x: 10, y: 20 });
    expect(canvasNode.title).toBe('Localized title');
  });

  it('constructs the candidate bucket before entering the Zustand setter', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection(), 1);
    const previous = useGraphDataStore.getState().graphEntities['functions/main'];
    const malformed = projection('functions/main', 5);
    Object.defineProperty(malformed.nodes[0].ports[0], 'kind', {
      get: () => {
        throw new Error('candidate conversion failed');
      },
    });

    const result = store.replaceProjection('functions/main', malformed, 2);
    expect(result).toMatchObject({
      applied: false,
      reason: 'invalid',
      error: expect.any(Error),
    });
    expect((result as { error?: Error }).error?.message).toBe('candidate conversion failed');
    expect(useGraphDataStore.getState().graphEntities['functions/main']).toBe(previous);
  });
  beforeEach(() => {
    useGraphDataStore.setState({ graphEntities: {} });
  });

  it('atomically replaces a graph with projected canvas entities and metadata', () => {
    const nextProjection = projection();

    const result = useGraphDataStore
      .getState()
      .replaceProjection('functions/main', nextProjection, 1);

    const bucket = useGraphDataStore.getState().graphEntities['functions/main'];
    expect(result).toEqual({ applied: true, reason: 'newer' });
    expect(bucket.sourceRevision).toBe(4);
    expect(bucket.requestGeneration).toBe(1);
    expect(bucket.nodes['shared-node']).toMatchObject({
      nodeType: 'unknown.projected-node',
      title: 'Localized title',
      description: 'Projected description',
      display: {
        styleId: 'projected-style',
        iconId: 'projected-icon',
      },
    });
    expect(bucket.pins[portAddressKey(input)]).toMatchObject({
      id: portAddressKey(input),
      address: input,
      instanceKind: 'userCreated',
      canRemove: true,
    });
    expect(bucket.connections['connection-1'].from).toBe(portAddressKey(output));
    expect(bucket.diagnostics).toEqual(nextProjection.diagnostics);
    const state = useGraphDataStore.getState();
    expect(getGraphProjectionBasis(state, 'functions/main')).toEqual(nextProjection.basis);
    expect(getGraphSourceRevision(state, 'functions/main')).toBe(4);
    expect(getGraphRequestGeneration(state, 'functions/main')).toBe(1);
    expect(getGraphDiagnostics(state, 'functions/main')).toEqual(nextProjection.diagnostics);
    expect(hasGraphBlockingDiagnostics(state, 'functions/main')).toBe(false);

    const canvasNode = toUiNode(bucket.nodes['shared-node'], {
      pins: bucket.nodePins['shared-node'].map((key) => ({
        pin: bucket.pins[key],
        connectionIds: bucket.pinConnections[key],
      })),
    });
    expect(canvasNode).toMatchObject({
      nodeType: 'unknown.projected-node',
      title: 'Localized title',
      description: 'Projected description',
      uiStyle: 'projected-style',
    });
  });

  it('ignores a lower source revision even from a newer request generation', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection('functions/main', 4), 2);
    const previous = useGraphDataStore.getState().graphEntities['functions/main'];

    const result = store.replaceProjection('functions/main', projection('functions/main', 3), 3);

    expect(result.applied).toBe(false);
    expect(useGraphDataStore.getState().graphEntities['functions/main']).toBe(previous);
  });

  it('ignores older request generations even when their revision is higher', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection('functions/main', 4), 2);
    const previous = useGraphDataStore.getState().graphEntities['functions/main'];

    const result = store.replaceProjection('functions/main', projection('functions/main', 5), 1);

    expect(result.applied).toBe(false);
    expect(useGraphDataStore.getState().graphEntities['functions/main']).toBe(previous);
  });

  it('allows a newer generation to replace same-revision localized display data', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection('functions/main', 4, 'English'), 1);

    const result = store.replaceProjection(
      'functions/main',
      projection('functions/main', 4, '本地化标题'),
      2,
    );

    expect(result).toEqual({ applied: true, reason: 'newer' });
    expect(useGraphDataStore.getState().graphEntities['functions/main'].nodes['shared-node'].title)
      .toBe('本地化标题');
  });

  it('leaves the previous bucket byte-for-byte unchanged for malformed projections', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/main', projection(), 1);
    const previous = useGraphDataStore.getState().graphEntities['functions/main'];
    const previousBytes = JSON.stringify(previous);
    const malformed = projection('functions/main', 5);
    malformed.connections[0].input = {
      kind: 'declared',
      nodeId: 'missing-node',
      portKey: 'missing-port',
    };

    const result = store.replaceProjection('functions/main', malformed, 2);

    expect(result.applied).toBe(false);
    expect(useGraphDataStore.getState().graphEntities['functions/main']).toBe(previous);
    expect(JSON.stringify(previous)).toBe(previousBytes);
  });

  it('isolates overlapping projected node ids by graphPath', () => {
    const store = useGraphDataStore.getState();
    store.replaceProjection('functions/first', projection('functions/first', 1, 'First'), 1);
    store.replaceProjection('functions/second', projection('functions/second', 1, 'Second'), 1);

    expect(store.getGraphNode('functions/first', 'shared-node')?.title).toBe('First');
    expect(store.getGraphNode('functions/second', 'shared-node')?.title).toBe('Second');
  });

  it('installs two valid projection replacements in one store update', () => {
    const firstPath = 'functions/first';
    const secondPath = 'functions/second';
    const updates = vi.fn();
    const unsubscribe = useGraphDataStore.subscribe(updates);

    const result = useGraphDataStore.getState().replaceProjectionsAtomically([
      { graphPath: firstPath, projection: projection(firstPath, 1, 'First') },
      { graphPath: secondPath, projection: projection(secondPath, 1, 'Second') },
    ]);

    unsubscribe();
    expect(result).toEqual({ applied: true, graphPaths: [firstPath, secondPath] });
    expect(updates).toHaveBeenCalledTimes(1);
    expect(useGraphDataStore.getState().graphEntities[firstPath].nodes['shared-node'].title).toBe('First');
    expect(useGraphDataStore.getState().graphEntities[secondPath].nodes['shared-node'].title).toBe('Second');
  });

  it('installs zero projection replacements when one candidate is malformed', () => {
    const firstPath = 'functions/first';
    const secondPath = 'functions/second';
    const store = useGraphDataStore.getState();
    store.replaceProjection(firstPath, projection(firstPath, 1, 'Current first'), 1);
    store.replaceProjection(secondPath, projection(secondPath, 1, 'Current second'), 1);
    const previousFirst = useGraphDataStore.getState().graphEntities[firstPath];
    const previousSecond = useGraphDataStore.getState().graphEntities[secondPath];
    const malformed = projection(secondPath, 2, 'Malformed second');
    malformed.connections[0].input = {
      kind: 'declared',
      nodeId: 'missing-node',
      portKey: 'missing-port',
    };
    const updates = vi.fn();
    const unsubscribe = useGraphDataStore.subscribe(updates);

    const result = useGraphDataStore.getState().replaceProjectionsAtomically([
      { graphPath: firstPath, projection: projection(firstPath, 2, 'Replacement first') },
      { graphPath: secondPath, projection: malformed },
    ]);

    unsubscribe();
    expect(result).toMatchObject({ applied: false, reason: 'invalid' });
    expect(updates).not.toHaveBeenCalled();
    expect(useGraphDataStore.getState().graphEntities[firstPath]).toBe(previousFirst);
    expect(useGraphDataStore.getState().graphEntities[secondPath]).toBe(previousSecond);
  });
});
