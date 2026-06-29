import { describe, expect, it } from 'vitest';
import type { ConnectionItem, Pin } from '@/shared/types/domain';
import type { Node } from '@/shared/types/ui';
import { validateConnections } from './connections';
import { setActiveTypeSystem, type TypeSystemSnapshot } from '@/shared/types/domain/typeSystem';

const TYPE_SYSTEM: TypeSystemSnapshot = {
  structTypes: {
    Model: { key: 'Model', parents: [], category: 'model' },
    OLSModel: { key: 'OLSModel', parents: ['Model'], category: 'model' },
  },
};

function pin(partial: Partial<Pin> & Pick<Pin, 'id' | 'nodeId' | 'direction'>): Pin {
  return {
    name: partial.id,
    type: 'struct',
    ...partial,
  } as Pin;
}

describe('validateConnections', () => {
  it('uses structured TypeSystem matching instead of flat pin.type equality', () => {
    setActiveTypeSystem(TYPE_SYSTEM);

    const out = pin({
      id: 'out',
      nodeId: 'ols',
      direction: 'output',
      dataType: { kind: 'Struct', inner: 'OLSModel' },
    });
    const input = pin({
      id: 'in',
      nodeId: 'predict',
      direction: 'input',
      dataType: { kind: 'Struct', inner: 'Model' },
    });
    const nodes = [
      { id: 'ols', inputs: [], outputs: [out] },
      { id: 'predict', inputs: [input], outputs: [] },
    ] as Node[];
    const connections: ConnectionItem[] = [{ fromPin: 'out', toPin: 'in' }];

    expect(validateConnections(connections, nodes)).toEqual([]);
  });
});
