import { describe, expect, it } from 'vitest';
import semanticProtocol from '@/tests/fixtures/node-system-contracts/semantic-protocol.json';
import i18nInventory from '@/tests/fixtures/node-system-contracts/i18n-inventory.json';
import localizedCatalog from '@/tests/fixtures/node-system-contracts/localized-catalog.json';
import editorProjection from '@/tests/fixtures/node-system-contracts/editor-projection.json';
import fingerprintWire from '@/tests/fixtures/node-system-contracts/fingerprint-wire.json';
import projectEvents from '@/tests/fixtures/node-system-contracts/project-events.json';
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

function clone<T>(value: T): T {
  return structuredClone(value);
}

function deleteKey(value: object, key: string): void {
  delete (value as Record<string, unknown>)[key];
}

const fingerprintPattern = /^[0-9a-f]{64}$/;

describe('Rust-generated node-system golden contracts', () => {
  it('consumes the exact production GraphDelta and ResourceMutationCommitted event shapes', () => {
    expect(projectEvents.format).toBe('yssbi.project-events.v1');
    expect(projectEvents.events.map((event) => [event.type, event.payload.type])).toEqual([
      ['Project', 'GraphDelta'],
      ['Project', 'ResourceMutationCommitted'],
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
      fingerprintWire.runEvent,
      fingerprintWire.trace,
    ].every((value) => fingerprintPattern.test(value))).toBe(true);
    expect(new Set([
      i18nInventory.registryFingerprint,
      localizedCatalog.registryFingerprint,
      editorProjection.basis.registryFingerprint,
      fingerprintWire.catalog,
      fingerprintWire.editorProjection,
      fingerprintWire.runEvent,
      fingerprintWire.trace,
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
      (wrong.items[index].creation as { kind: string }).kind = 'legacy';
      expect(isLocalizedCatalogDto(wrong)).toBe(false);
    },
  );

  it('accepts the authoritative editor projection through the real strict parser', () => {
    expect(isEditorGraphProjectionDto(editorProjection)).toBe(true);
    expect(parseEditorGraphProjectionDto(editorProjection)).toEqual(editorProjection);
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

      Object.assign(value, { compatibility: true });
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      deleteKey(value, 'compatibility');
      deleteKey(value, missing);
      expect(isEditorGraphProjectionDto(projection)).toBe(false);
      value.kind = 'legacy';
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
      value.kind = 'legacy';
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
      value.kind = 'legacy';
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
      nodes[0].legacy = true;
    }],
    ['missing port key', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      deleteKey((node.ports as Array<Record<string, unknown>>)[0], 'status');
    }],
    ['wrong address discriminant', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      const port = (node.ports as Array<Record<string, unknown>>)[0];
      (port.address as Record<string, unknown>).kind = 'legacy';
    }],
    ['wrong diagnostic location discriminant', (projection: Record<string, unknown>) => {
      projection.diagnostics = [{
        code: 'contract.invalid', message: 'invalid', severity: 'error', blocking: true,
        location: { kind: 'legacy' }, related: [],
      }];
    }],
    ['wrong configuration discriminant', (projection: Record<string, unknown>) => {
      const node = (projection.nodes as Array<Record<string, unknown>>)[0];
      const editor = (node.parameterEditors as Array<Record<string, unknown>>)[0];
      editor.configuration = { kind: 'legacy' };
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
