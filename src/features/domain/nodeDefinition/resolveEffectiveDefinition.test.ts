import { describe, expect, it } from 'vitest';
import type { NodeDefinition } from '@/shared/types/domain/node';
import {
  CALL_FUNCTION_NODE_TYPE,
  defaultFunctionSignature,
  resolveEffectiveDefinition,
  signatureToPinSlots,
} from './resolveEffectiveDefinition';

const callBase: NodeDefinition = {
  name: 'Call Function',
  category: ['Functions'],
  nodeType: CALL_FUNCTION_NODE_TYPE,
  nodeMetadata: {
    uiStyle: 'function',
    supports_dynamic_pins: true,
    graph_scope: 'any',
    shell_role: null,
  },
  pinSlots: [],
  typeCapabilities: [],
};

describe('resolveEffectiveDefinition', () => {
  it('returns base unchanged for non-call nodes', () => {
    const ols = { ...callBase, nodeType: 'Data:OLS', pinSlots: [{ slotKind: 'fixed' as const, pin: {} as never }] };
    expect(resolveEffectiveDefinition(ols, { subGraphId: 'x' })).toBe(ols);
  });

  it('projects default exec in/out pinSlots for call function', () => {
    const defaults = defaultFunctionSignature();
    const effective = resolveEffectiveDefinition(callBase, {
      subGraphId: 'fn-1',
      ...defaults,
    });

    expect(effective.pinSlots).toHaveLength(2);
    expect(effective.pinSlots[0].slotKind).toBe('fixed');
    if (effective.pinSlots[0].slotKind === 'fixed') {
      expect(effective.pinSlots[0].pin).toMatchObject({
        name: 'In',
        direction: 'input',
        kind: 'Exec',
      });
    }
    expect(effective.typeCapabilities).toHaveLength(2);
  });

  it('signatureToPinSlots maps data pin with concrete type', () => {
    const slots = signatureToPinSlots(
      [{ id: 'a', name: 'Value', type: 'float' }],
      [],
    );
    expect(slots).toHaveLength(1);
    if (slots[0].slotKind === 'fixed') {
      expect(slots[0].pin.dataType).toEqual({ Concrete: { kind: 'Float64' } });
    }
  });
});
