import { describe, expect, it } from 'vitest';
import {
  applyVariableCatalogFromIndex,
  variableCatalogToResourceMetas,
  variableFromIndexRow,
} from '@/features/core/variable/variableCatalog';

describe('variableCatalog', () => {
  it('hydrates VariableStore entries from ProjectIndex.variables rows', () => {
    const catalog = applyVariableCatalogFromIndex([
      {
        id: 'global-1',
        name: 'Counter',
        dataType: { kind: 'Int32' },
        dataValue: { kind: 'Int32', value: 0 },
        description: '',
        scope: { type: 'global' },
        tags: [],
      },
      {
        id: 'local-1',
        name: 'Temp',
        dataType: { kind: 'Float64' },
        dataValue: { kind: 'Float64', value: 1.5 },
        description: '',
        scope: { type: 'event', eventId: 'graph-a' },
        tags: [],
        ownerGraphId: 'graph-a',
      },
    ]);

    expect(Object.keys(catalog)).toEqual(['global-1', 'local-1']);
    expect(variableFromIndexRow({
      id: 'global-1',
      name: 'Counter',
      dataType: { kind: 'Int32' },
      dataValue: { kind: 'Int32', value: 0 },
      description: '',
      scope: { type: 'global' },
      tags: [],
    }).name).toBe('Counter');
  });

  it('projects variable catalog entries to resource metas', () => {
    const metas = variableCatalogToResourceMetas({
      'v1': {
        id: 'v1',
        name: 'Alpha',
        dataType: { kind: 'Boolean' },
        dataValue: { kind: 'Boolean', value: false },
        description: '',
        scope: { type: 'function', functionId: 'fn-1' },
        tags: [],
      },
    });

    expect(metas).toHaveLength(1);
    expect(metas[0]).toMatchObject({
      id: 'v1',
      kind: 'variable',
      name: 'Alpha',
      scope: { type: 'function', graphId: 'fn-1' },
    });
  });
});
