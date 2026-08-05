import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import type {
  EditorGraphProjectionDto,
  PortAddressDto,
} from '@/shared/types/dto/editorProjection';
import {
  portAddressKey,
  toProjectionEntities,
  validateEditorGraphProjection,
} from './index';

const declaredOutput: PortAddressDto = {
  kind: 'declared',
  nodeId: 'node-1',
  portKey: 'output',
};

const instanceInput: PortAddressDto = {
  kind: 'instance',
  nodeId: 'node-1',
  templateKey: 'input',
  instanceId: 'instance-1',
};

function validProjection(): EditorGraphProjectionDto {
  return {
    basis: {
      graphPath: 'functions/main',
      graphRevision: 7,
      registryFingerprint: Array.from({ length: 32 }, () => 1),
      resourceVersions: { 'functions/helper': '3' },
    },
    graphPath: 'functions/main',
    sourceRevision: 7,
    nodes: [
      {
        graphPath: 'functions/main',
        sourceRevision: 7,
        nodeId: 'node-1',
        nodeTypeId: 'statistics.linear-regression',
        position: { x: 120.5, y: -32 },
        display: {
          title: '线性回归',
          description: '拟合线性模型',
          userLabel: '主要模型',
          iconId: 'chart-line',
          styleId: 'analysis',
        },
        ports: [
          {
            address: declaredOutput,
            templateKey: 'output',
            display: { label: '结果', instanceLabel: null },
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
            resolvedType: { display: 'Model', resolved: true },
            resolvedSchema: { kind: 'derived', fields: [] },
            status: 'resolved',
          },
          {
            address: instanceInput,
            templateKey: 'input',
            display: { label: '变量', instanceLabel: '变量 1' },
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
              literalOverride: 42,
              protocolDefault: 0,
              effective: 'connections',
            },
            resolvedType: { display: 'Float64', resolved: true },
            resolvedSchema: { kind: 'input', fields: [] },
            status: 'resolved',
          },
        ],
        parameterEditors: [
          {
            key: 'formula',
            display: { title: '公式', description: '模型公式' },
            editor: 'text',
            multiline: true,
            value: 'y ~ x',
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
        diagnostics: [
          {
            code: 'node.warning',
            message: '节点警告',
            severity: 'warning',
            blocking: false,
            location: { kind: 'port', address: instanceInput },
            related: [{ kind: 'node', nodeId: 'node-1' }],
          },
        ],
      },
    ],
    connections: [
      {
        connectionId: 'connection-1',
        output: declaredOutput,
        input: instanceInput,
        order: 'a',
      },
    ],
    diagnostics: [
      {
        code: 'graph.info',
        message: '图诊断',
        severity: 'information',
        blocking: false,
        location: { kind: 'graph' },
        related: [{ kind: 'resource', identity: 'functions/helper' }],
      },
    ],
    hasBlockingDiagnostics: false,
  };
}

describe('editor projection architecture', () => {
  it('keeps domain production modules independent from services', () => {
    const productionModules = [
      'portAddressKey.ts',
      'toProjectionEntities.ts',
      'types.ts',
      'validateProjection.ts',
    ];
    const serviceImport = /from\s+['"]@\/services(?:\/|['"])/;
    const offenders = productionModules.filter((fileName) => {
      const source = readFileSync(new URL(fileName, import.meta.url), 'utf8');
      return serviceImport.test(source);
    });

    expect(offenders).toEqual([]);
  });
});



describe('portAddressKey', () => {
  it('is stable for equal addresses and distinguishes address variants', () => {
    expect(portAddressKey(declaredOutput)).toBe(portAddressKey({ ...declaredOutput }));
    expect(portAddressKey(declaredOutput)).not.toBe(portAddressKey(instanceInput));
  });

  it('does not collide when address parts contain delimiters', () => {
    const first: PortAddressDto = {
      kind: 'declared',
      nodeId: 'a:b',
      portKey: 'c',
    };
    const second: PortAddressDto = {
      kind: 'declared',
      nodeId: 'a',
      portKey: 'b:c',
    };

    expect(portAddressKey(first)).not.toBe(portAddressKey(second));
  });
});

describe('validateEditorGraphProjection', () => {
  it('returns a valid projection unchanged', () => {
    const projection = validProjection();
    expect(validateEditorGraphProjection(projection)).toBe(projection);
  });

  it.each([
    ['basis graph path', (projection: EditorGraphProjectionDto) => {
      projection.basis.graphPath = 'functions/other';
    }],
    ['node graph path', (projection: EditorGraphProjectionDto) => {
      projection.nodes[0].graphPath = 'functions/other';
    }],
    ['basis revision', (projection: EditorGraphProjectionDto) => {
      projection.basis.graphRevision = 8;
    }],
    ['node revision', (projection: EditorGraphProjectionDto) => {
      projection.nodes[0].sourceRevision = 8;
    }],
  ])('rejects mismatched %s', (_, mutate) => {
    const projection = validProjection();
    mutate(projection);
    expect(() => validateEditorGraphProjection(projection)).toThrow(/does not match/);
  });

  it('rejects duplicate node, port, and connection identities', () => {
    const duplicateNode = validProjection();
    duplicateNode.nodes.push(structuredClone(duplicateNode.nodes[0]));
    expect(() => validateEditorGraphProjection(duplicateNode)).toThrow(/duplicate node/);

    const duplicatePort = validProjection();
    duplicatePort.nodes[0].ports.push(structuredClone(duplicatePort.nodes[0].ports[0]));
    expect(() => validateEditorGraphProjection(duplicatePort)).toThrow(/duplicate port/);

    const duplicateConnection = validProjection();
    duplicateConnection.connections.push(structuredClone(duplicateConnection.connections[0]));
    expect(() => validateEditorGraphProjection(duplicateConnection)).toThrow(/duplicate connection/);
  });

  it('strictly validates Rust-issued schema-aware editor wire data', () => {
    const projection = validProjection();
    projection.nodes[0].parameterEditors[0].configuration = {
      kind: 'filterPredicate',
      available: true,
      unavailableReason: null,
      columns: [{
        name: 'amount',
        dataType: 'float64',
        operators: ['equal', 'greaterThan', 'isNull'],
        literalTypes: ['integer', 'decimal'],
      }],
      value: {
        column: 'amount',
        operator: 'greaterThan',
        value: { type: 'decimal', value: '9007199254740993.5' },
      },
    };
    expect(validateEditorGraphProjection(projection)).toBe(projection);

    const extra = structuredClone(projection);
    Object.assign(extra.nodes[0].parameterEditors[0].configuration!, { compatibility: true });
    expect(() => validateEditorGraphProjection(extra)).toThrow(/parameter editor/);

    const lossy = structuredClone(projection);
    const configuration = lossy.nodes[0].parameterEditors[0].configuration;
    if (configuration?.kind !== 'filterPredicate' || !configuration.value?.value) {
      throw new Error('test fixture mismatch');
    }
    configuration.value.value.value = 9007199254740994 as never;
    expect(() => validateEditorGraphProjection(lossy)).toThrow(/parameter editor/);
  });

  it('rejects a port address owned by a different node', () => {
    const projection = validProjection();
    projection.nodes[0].ports[0].address = {
      ...declaredOutput,
      nodeId: 'node-2',
    };

    expect(() => validateEditorGraphProjection(projection)).toThrow(/owned by node 'node-2'/);
  });

  it('rejects connections that reference a missing port', () => {
    const missingEndpointProjection = validProjection();
    missingEndpointProjection.connections[0].input = {
      kind: 'declared',
      nodeId: 'node-1',
      portKey: 'missing',
    };

    expect(() => validateEditorGraphProjection(missingEndpointProjection)).toThrow(
      "projection connection 'connection-1' references a missing port",
    );
  });

  it.each([
    ['output', 0, 'input'],
    ['input', 1, 'output'],
  ] as const)('rejects a connection whose %s endpoint has the wrong direction', (_, portIndex, direction) => {
    const wrongDirectionProjection = validProjection();
    wrongDirectionProjection.nodes[0].ports[portIndex].direction = direction;

    expect(() => validateEditorGraphProjection(wrongDirectionProjection)).toThrow(
      /connection 'connection-1'.*direction/,
    );
  });
});

describe('toProjectionEntities', () => {
  it('converts a valid projection without a registry and preserves projected data', () => {
    const projection = validProjection();
    const entities = toProjectionEntities(projection);
    const outputKey = portAddressKey(declaredOutput);
    const inputKey = portAddressKey(instanceInput);

    expect(entities.basis).toEqual(projection.basis);
    expect(entities.graphPath).toBe('functions/main');
    expect(entities.sourceRevision).toBe(7);
    expect(entities.nodes['node-1']).toMatchObject({
      nodeTypeId: 'statistics.linear-regression',
      position: { x: 120.5, y: -32 },
      display: { title: '线性回归', userLabel: '主要模型' },
      parameterEditors: projection.nodes[0].parameterEditors,
      diagnostics: projection.nodes[0].diagnostics,
    });
    expect(entities.ports[outputKey].address).toEqual(declaredOutput);
    expect(entities.ports[inputKey]).toMatchObject({
      address: instanceInput,
      input: {
        literalOverride: 42,
        protocolDefault: 0,
        effective: 'connections',
      },
    });
    expect(entities.connections['connection-1']).toEqual(projection.connections[0]);
    expect(entities.portKeysByNodeId['node-1']).toEqual([outputKey, inputKey]);
    expect(entities.connectionIdsByPortKey[outputKey]).toEqual(['connection-1']);
    expect(entities.connectionIdsByPortKey[inputKey]).toEqual(['connection-1']);
    expect(entities.diagnostics).toEqual(projection.diagnostics);
    expect(entities.hasBlockingDiagnostics).toBe(false);
  });
});
