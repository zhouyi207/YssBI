import { describe, expect, it } from 'vitest';
import semanticProtocol from '@/tests/fixtures/node-system-contracts/semantic-protocol.json';
import i18nInventory from '@/tests/fixtures/node-system-contracts/i18n-inventory.json';
import localizedCatalog from '@/tests/fixtures/node-system-contracts/localized-catalog.json';
import editorProjection from '@/tests/fixtures/node-system-contracts/editor-projection.json';
import fingerprintWire from '@/tests/fixtures/node-system-contracts/fingerprint-wire.json';
import functionEditorProjection from '@/tests/fixtures/node-system-contracts/function-editor-projection.json';
import projectEvents from '@/tests/fixtures/node-system-contracts/project-events.json';
import executionWire from '@/tests/fixtures/node-system-contracts/execution-wire.json';
import {
  isLocalizedCatalogDto,
  type LocalizedCatalogDto,
} from '@/shared/types/dto/localizedCatalog';
import { isNodeCreationDescriptorDto } from '@/shared/types/dto/nodeCreationDescriptor';
import { isEditorGraphProjectionDto } from '@/shared/types/dto/editorProjectionGuards';
import { parseEditorGraphProjectionDto } from '@/shared/types/dto/editorProjectionParser';
import { isSchemaAwareParameterEditorDto } from '@/shared/types/dto/parameterEditorValidators';
import {
  parseProjectMutationEvent,
} from '@/features/core/sync/utils/projectEventWireParser';
import { parseProjectGraphIndexRow } from '@/services/project/projectService';
import { parseGraphProjectionReplacementDto } from '@/shared/types/dto/editorMutationWireParser';
import { parseRunEvent } from '@/shared/types/dto/runEventParser';

function clone<T>(value: T): T {
  return structuredClone(value);
}

function deleteKey(value: object, key: string): void {
  delete (value as Record<string, unknown>)[key];
}

const fingerprintPattern = /^[0-9a-f]{64}$/;

describe('Rust-generated node-system golden contracts', () => {
  it('strictly freezes every Rust deadline phase before service effects', () => {
    const deadlineEvents = executionWire.runEvents.filter(
      (event) => event.kind.type === 'runErrored' && event.kind.code === 'deadlineExceeded',
    );
    expect(deadlineEvents.map((event) => event.kind.phase)).toEqual([
      'queueWait',
      'kernel',
      'streamSend',
      'streamReceive',
      'adapterIo',
      'resultPublication',
      'cleanup',
    ]);
    expect(deadlineEvents.map(parseRunEvent)).toEqual(deadlineEvents);

    for (const event of deadlineEvents) {
      const missing = clone(event) as unknown as Record<string, unknown>;
      deleteKey(missing.kind as object, 'phase');
      expect(() => parseRunEvent(missing)).toThrow();

      const wrong = clone(event) as unknown as Record<string, unknown>;
      (wrong.kind as Record<string, unknown>).phase = 1;
      expect(() => parseRunEvent(wrong)).toThrow();
    }
  });
  it('consumes one real Rust function editor projection shape across index and replacement', () => {
    expect(functionEditorProjection.format).toBe('yssbi.function-editor-projection.v1');
    const row = parseProjectGraphIndexRow(functionEditorProjection.indexRow);
    const replacement = parseGraphProjectionReplacementDto(functionEditorProjection.replacement);

    expect(row.type).toBe('function');
    if (row.type !== 'function') throw new Error('expected function row');
    expect(replacement).toHaveProperty('functionEditorProjection');
    if (!('functionEditorProjection' in replacement)) {
      throw new Error('expected function replacement');
    }
    expect(row.functionEditorProjection).toEqual(replacement.functionEditorProjection);
    expect(row.functionEditorProjection.outputs[0].name).toBe('Array<String>');
  });

  it('consumes the exact production GraphDelta and ResourceMutationCommitted event shapes', () => {
    expect(projectEvents.format).toBe('yssbi.project-events.v1');
    expect(projectEvents.events.map((event) => [event.type, event.payload.type])).toEqual([
      ['Project', 'GraphDelta'],
      ...projectEvents.resourceMutationResults.map(() => ['Project', 'ResourceMutationCommitted']),
    ]);
    expect(projectEvents.resourceMutationResults.map(({ scenario }) => scenario)).toEqual([
      'create', 'save', 'rename', 'remove', 'undo', 'redo',
    ]);
    expect(projectEvents.events.map(parseProjectMutationEvent)).toEqual(projectEvents.events);
  });

  it('shares one canonical Registry fingerprint across every wire purpose', () => {
    expect(semanticProtocol.format).toBe('yssbi.semantic-node-protocol.v1');
    expect(i18nInventory.format).toBe('yssbi.i18n-inventory.v1');
    expect(i18nInventory.defaultLocale).toBe('en-US');
    expect(i18nInventory.requiredKeys.length).toBeGreaterThan(0);
    expect(i18nInventory.aliasKeys.length).toBeGreaterThan(0);
    expect(fingerprintWire.format).toBe('yssbi.registry-fingerprint-wire.v1');
    expect([
      fingerprintWire.catalog,
      fingerprintWire.editorProjection,
    ].every((value) => fingerprintPattern.test(value))).toBe(true);
    expect(new Set([
      i18nInventory.registryFingerprint,
      localizedCatalog.registryFingerprint,
      editorProjection.basis.registryFingerprint,
      fingerprintWire.catalog,
      fingerprintWire.editorProjection,
    ])).toEqual(new Set([fingerprintWire.catalog]));
  });

  it('accepts the authoritative localized Catalog and every descriptor variant', () => {
    expect(isLocalizedCatalogDto(localizedCatalog)).toBe(true);
    expect(localizedCatalog.items.map((item) => item.creation.kind)).toEqual([
      'static',
      'parameterizedStatic',
      'resourceBound',
      'resourceBound',
      'resourceBound',
    ]);
    for (const item of localizedCatalog.items) {
      expect(isNodeCreationDescriptorDto(item.creation)).toBe(true);
    }
  });

  it('accepts node catalog items with documentation but no short description', () => {
    const catalog = clone(localizedCatalog) as unknown as Record<string, unknown>;
    const items = catalog.items as Array<Record<string, unknown>>;
    items.forEach((item) => deleteKey(item, 'description'));

    expect(isLocalizedCatalogDto(catalog)).toBe(true);
    expect(items.every((item) => !Object.prototype.hasOwnProperty.call(item, 'description')))
      .toBe(true);
  });

  it.each([
    ['unknown key', (catalog: Record<string, unknown>) => { catalog.compatibility = true; }],
    ['missing key', (catalog: Record<string, unknown>) => { deleteKey(catalog, 'locale'); }],
    ['number array fingerprint', (catalog: Record<string, unknown>) => {
      catalog.registryFingerprint = Array(32).fill(1);
    }],
    ['uppercase fingerprint', (catalog: Record<string, unknown>) => {
      catalog.registryFingerprint = localizedCatalog.registryFingerprint.toUpperCase();
    }],
    ['short fingerprint', (catalog: Record<string, unknown>) => {
      catalog.registryFingerprint = localizedCatalog.registryFingerprint.slice(1);
    }],
  ])('rejects Catalog %s', (_label, mutate) => {
    const catalog = clone(localizedCatalog) as unknown as Record<string, unknown>;
    mutate(catalog);
    expect(isLocalizedCatalogDto(catalog)).toBe(false);
  });

  it.each(localizedCatalog.items.map((item, index) => [item.creation.kind, index] as const))(
    'rejects unknown, missing, and wrong %s descriptor variants',
    (_kind, index) => {
      const unknown = clone(localizedCatalog) as unknown as LocalizedCatalogDto;
      Object.assign(unknown.items[index].creation, { compatibility: true });
      expect(isLocalizedCatalogDto(unknown)).toBe(false);

      const missing = clone(localizedCatalog) as unknown as LocalizedCatalogDto;
      deleteKey(missing.items[index].creation, 'nodeTypeId');
      expect(isLocalizedCatalogDto(missing)).toBe(false);

      const wrong = clone(localizedCatalog) as unknown as LocalizedCatalogDto;
      (wrong.items[index].creation as { kind: string }).kind = 'unsupported';
      expect(isLocalizedCatalogDto(wrong)).toBe(false);
    },
  );

  it('freezes the six-field authoritative port connection capability', () => {
    expect(editorProjection.nodes[0].ports[0].connections).toEqual({
      current: 0,
      maximum: 1,
      ordered: false,
      canAppend: true,
      canReplace: false,
      canMove: false,
    });
  });

  it('accepts the authoritative editor projection through the real strict parser', () => {
    expect(editorProjection).toHaveProperty('outcome', { type: 'success' });
    expect(isEditorGraphProjectionDto(editorProjection)).toBe(true);
    expect(parseEditorGraphProjectionDto(editorProjection)).toEqual(editorProjection);
  });

  it('strictly parses typed projection compilation outcomes', () => {
    const internal = clone(editorProjection) as unknown as Record<string, unknown>;
    internal.outcome = {
      type: 'internalFailure',
      stage: 'lowering',
      code: 'compiler.lowering.internal_invariant',
      nodeId: editorProjection.nodes[0].nodeId,
    };
    internal.hasBlockingDiagnostics = true;
    expect(isEditorGraphProjectionDto(internal)).toBe(true);
    expect(parseEditorGraphProjectionDto(internal).outcome).toEqual(internal.outcome);

    internal.hasBlockingDiagnostics = false;
    expect(isEditorGraphProjectionDto(internal)).toBe(false);

    const missing = clone(editorProjection) as unknown as Record<string, unknown>;
    deleteKey(missing, 'outcome');
    expect(isEditorGraphProjectionDto(missing)).toBe(false);
  });

  it('strictly accepts and rejects every projection address variant', () => {
    const variants = [
      {
        value: {
          kind: 'declared',
          nodeId: editorProjection.nodes[0].nodeId,
          portKey: 'value',
        },
        missing: 'portKey',
      },
      {
        value: {
          kind: 'instance',
          nodeId: editorProjection.nodes[0].nodeId,
          templateKey: 'value',
          instanceId: '00000000-0000-0000-0000-000000000005',
        },
        missing: 'instanceId',
      },
    ];
    for (const { value, missing } of variants) {
      const projection = clone(editorProjection) as unknown as Record<string, unknown>;
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      (node.ports as Array<Record<string, unknown>>)[0].address = value;
      expect(isEditorGraphProjectionDto(projection)).toBe(true);

      const validNodeId = value.nodeId;
      value.nodeId = 'not-a-uuid';
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      value.nodeId = validNodeId;
      if (value.kind === 'instance') {
        const validInstanceId = value.instanceId;
        Object.assign(value, { instanceId: 'not-a-uuid' });
        expect(isEditorGraphProjectionDto(projection)).toBe(false);
        Object.assign(value, { instanceId: validInstanceId });
      }

      Object.assign(value, { compatibility: true });
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      deleteKey(value, 'compatibility');
      deleteKey(value, missing);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      value.kind = 'unsupported';
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
    }
  });

  it('strictly accepts and rejects every diagnostic location variant', () => {
    const variants = [
      { value: { kind: 'graph' }, missing: 'kind' },
      { value: { kind: 'node', nodeId: editorProjection.nodes[0].nodeId }, missing: 'nodeId' },
      {
        value: { kind: 'port', address: editorProjection.nodes[0].ports[0].address },
        missing: 'address',
      },
      { value: { kind: 'connection', connectionId: 'contract-connection' }, missing: 'connectionId' },
      {
        value: { kind: 'parameter', nodeId: editorProjection.nodes[0].nodeId, key: 'value' },
        missing: 'key',
      },
      { value: { kind: 'resource', identity: 'functions/contract-function' }, missing: 'identity' },
    ];
    for (const { value, missing } of variants) {
      const projection = clone(editorProjection) as unknown as Record<string, unknown>;
      projection.diagnostics = [{
        code: 'contract.variant', message: 'variant', severity: 'information', blocking: false,
        location: value, related: [],
      }];
      expect(isEditorGraphProjectionDto(projection)).toBe(true);

      Object.assign(value, { compatibility: true });
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      deleteKey(value, 'compatibility');
      deleteKey(value, missing);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      value.kind = 'unsupported';
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
    }
  });

  it('strictly accepts and rejects every schema-aware parameter configuration variant', () => {
    const variants = [
      {
        value: {
          kind: 'projectColumns', available: true, unavailableReason: null,
          options: [{ name: 'value', dataType: 'boolean' }], value: ['value'],
        },
        missing: 'options',
      },
      {
        value: {
          kind: 'filterPredicate', available: true, unavailableReason: null,
          columns: [{
            name: 'value', dataType: 'boolean', operators: ['equal'], literalTypes: ['boolean'],
          }],
          value: {
            column: 'value', operator: 'equal', value: { type: 'boolean', value: true },
          },
        },
        missing: 'columns',
      },
    ];
    for (const { value, missing } of variants) {
      const projection = clone(editorProjection) as unknown as Record<string, unknown>;
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      (node.parameterEditors as Array<Record<string, unknown>>)[0].configuration = value;
      expect(isSchemaAwareParameterEditorDto(value)).toBe(true);
      expect(isEditorGraphProjectionDto(projection)).toBe(true);

      Object.assign(value, { compatibility: true });
      expect(isSchemaAwareParameterEditorDto(value)).toBe(false);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      deleteKey(value, 'compatibility');
      deleteKey(value, missing);
      expect(isSchemaAwareParameterEditorDto(value)).toBe(false);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      value.kind = 'unsupported';
      expect(isSchemaAwareParameterEditorDto(value)).toBe(false);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
    }
  });

  it.each([
    ['root unknown key', (projection: Record<string, unknown>) => { projection.extra = true; }],
    ['root missing key', (projection: Record<string, unknown>) => {
      deleteKey(projection, 'hasBlockingDiagnostics');
    }],
    ['number array fingerprint', (projection: Record<string, unknown>) => {
      (projection.basis as Record<string, unknown>).registryFingerprint = Array(32).fill(1);
    }],
    ['uppercase fingerprint', (projection: Record<string, unknown>) => {
      (projection.basis as Record<string, unknown>).registryFingerprint =
        editorProjection.basis.registryFingerprint.toUpperCase();
    }],
    ['short fingerprint', (projection: Record<string, unknown>) => {
      (projection.basis as Record<string, unknown>).registryFingerprint =
        editorProjection.basis.registryFingerprint.slice(1);
    }],
    ['unknown node key', (projection: Record<string, unknown>) => {
      const nodes = projection.nodes as Array<Record<string, unknown>>;
      nodes[0].unexpected = true;
    }],
    ['missing port key', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      deleteKey((node.ports as Array<Record<string, unknown>>)[0], 'status');
    }],
    ['wrong address discriminant', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      const port = (node.ports as Array<Record<string, unknown>>)[0];
      (port.address as Record<string, unknown>).kind = 'unsupported';
    }],
    ['wrong diagnostic location discriminant', (projection: Record<string, unknown>) => {
      projection.diagnostics = [{
        code: 'contract.invalid', message: 'invalid', severity: 'error', blocking: true,
        location: { kind: 'unsupported' }, related: [],
      }];
    }],
    ['wrong configuration discriminant', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      const editor = (node.parameterEditors as Array<Record<string, unknown>>)[0];
      editor.configuration = { kind: 'unsupported' };
    }],
  ])('rejects editor projection %s before coherence validation', (_label, mutate) => {
    const projection = clone(editorProjection) as unknown as Record<string, unknown>;
    mutate(projection);
    expect(isEditorGraphProjectionDto(projection)).toBe(false);
    expect(() => parseEditorGraphProjectionDto(projection)).toThrow(
      'Invalid editor graph projection response',
    );
  });
});
