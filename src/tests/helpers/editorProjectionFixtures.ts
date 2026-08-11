import type {
  EditorGraphProjectionDto,
  PortAddressDto,
} from '@/shared/types/dto/editorProjection';
import { portAddressKey } from '@/features/domain/editorProjection';

export interface EditorProjectionFixtureOptions {
  graphPath: string;
  sourceRevision?: number;
  nodeId?: string;
  nodeTypeId?: string;
  title?: string;
  connectionId?: string;
}

export function makeEditorProjectionFixture(options: EditorProjectionFixtureOptions): {
  projection: EditorGraphProjectionDto;
  inputAddress: PortAddressDto;
  inputKey: string;
  outputAddress: PortAddressDto;
  outputKey: string;
} {
  const {
    graphPath,
    sourceRevision = 1,
    nodeId = 'local-node',
    nodeTypeId = 'tests.projected-node',
    title = 'Projected node',
    connectionId = 'local-connection',
  } = options;
  const outputAddress: PortAddressDto = {
    kind: 'declared',
    nodeId,
    portKey: 'local-out',
  };
  const inputAddress: PortAddressDto = {
    kind: 'declared',
    nodeId,
    portKey: 'local-in',
  };
  const outputKey = portAddressKey(outputAddress);
  const inputKey = portAddressKey(inputAddress);

  return {
    outputAddress,
    outputKey,
    inputAddress,
    inputKey,
    projection: {
      basis: {
        graphPath,
        graphRevision: sourceRevision,
        registryFingerprint: '0000000000000000000000000000000000000000000000000000000000000000',
        resourceVersions: {},
      },
      graphPath,
      sourceRevision,
      nodes: [
        {
          graphPath,
          sourceRevision,
          nodeId,
          nodeTypeId,
          position: { x: 0, y: 0 },
          display: {
            title,
            description: null,
            userLabel: null,
            iconId: null,
            styleId: null,
          },
          ports: [
            {
              address: outputAddress,
              templateKey: 'local-out',
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
              resolvedType: { display: 'Float64', resolved: true, dataType: { kind: 'Float64' } },
              resolvedSchema: null,
              status: 'resolved',
            },
            {
              address: inputAddress,
              templateKey: 'local-in',
              display: { label: 'Input', instanceLabel: null },
              direction: 'input',
              kind: 'data',
              instanceKind: 'declared',
              orphan: false,
              canRemove: false,
              connections: {
                current: 1,
                maximum: 1,
                ordered: false,
                canConnect: false,
              },
              input: {
                literalOverride: null,
                protocolDefault: null,
                effective: 'connections',
              },
              resolvedType: { display: 'Float64', resolved: true, dataType: { kind: 'Float64' } },
              resolvedSchema: null,
              status: 'resolved',
            },
          ],
          parameterEditors: [],
          capabilities: {
            managed: false,
            canCopy: true,
            canDelete: true,
            canEditLabel: true,
            canEditParameters: false,
            hasDynamicPorts: false,
            supportsInlineLiterals: false,
          },
          diagnostics: [],
        },
      ],
      connections: [
        {
          connectionId,
          output: outputAddress,
          input: inputAddress,
          order: null,
        },
      ],
      diagnostics: [],
      outcome: { type: 'success' },
      hasBlockingDiagnostics: false,
    },
  };
}
