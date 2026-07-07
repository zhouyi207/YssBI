import { describe, it, expect } from 'vitest';
import type { Pin, PinDirection } from '@/shared/types/domain/pin';
import type { DataType } from '@/shared/types/domain/dataType';
import type { TypeSystemSnapshot } from '@/shared/types/domain/typeSystem';
import { buildPinDataType, pinAcceptsType, isPinCompatible, canConnectPins, findAutoConnectPinIndex } from './pinCompatibility';
import { defaultFunctionSignature, resolveEffectiveDefinition, CALL_FUNCTION_NODE_TYPE } from '@/features/domain/nodeDefinition';

const FLOAT64: DataType = { kind: 'Float64' };
const STRING: DataType = { kind: 'String' };
const INT64: DataType = { kind: 'Int64' };
const SERIES_FLOAT64: DataType = { kind: 'DataSeries', inner: { kind: 'Float64' } };
const MODEL: DataType = { kind: 'Struct', inner: 'Model' };
const OLS_MODEL: DataType = { kind: 'Struct', inner: 'OLSModel' };
const OLS_RESULT: DataType = { kind: 'Struct', inner: 'OLSResult' };

const TYPE_SYSTEM: TypeSystemSnapshot = {
  structTypes: {
    Model: { key: 'Model', parents: [], category: 'model' },
    OLSModel: { key: 'OLSModel', parents: ['Model'], category: 'model' },
    OLSResult: { key: 'OLSResult', parents: [], category: 'result' },
  },
};

function pin(partial: Partial<Pin> & { direction: PinDirection }): Pin {
  return {
    id: partial.id ?? 'p1',
    nodeId: partial.nodeId ?? 'n1',
    name: partial.name ?? 'pin',
    type: partial.type ?? 'object',
    ...partial,
  } as Pin;
}

describe('buildPinDataType', () => {
  it('prefers the structured dataType over typeDisplay', () => {
    const p = pin({
      direction: 'output',
      type: 'number',
      typeDisplay: 'String', // intentionally conflicting display string
      dataType: SERIES_FLOAT64,
    });
    expect(buildPinDataType(p)).toEqual(SERIES_FLOAT64);
  });

  it('throws for data pins without structured dataType', () => {
    const p = pin({ direction: 'output', type: 'object', typeDisplay: 'Float64' });
    expect(() => buildPinDataType(p)).toThrow('missing structured dataType');
  });

  it('does not infer data pin types from type + containerType', () => {
    const p = pin({ direction: 'output', type: 'number', containerType: 'dataseries' });
    expect(() => buildPinDataType(p)).toThrow('missing structured dataType');
  });

  it('keeps exec pins outside the data type system', () => {
    const p = pin({ direction: 'output', type: 'exec' });
    expect(buildPinDataType(p)).toEqual({ kind: 'Any' });
  });
});

describe('pinAcceptsType - variable Set recommendations (dragging an output)', () => {
  const draggedSeriesOutput = pin({ direction: 'output', dataType: SERIES_FLOAT64 });

  it('recommends a DataSeries<Float64> variable', () => {
    expect(pinAcceptsType(draggedSeriesOutput, SERIES_FLOAT64)).toBe(true);
  });

  it('does not recommend a scalar Float64 variable for a DataSeries output', () => {
    expect(pinAcceptsType(draggedSeriesOutput, FLOAT64)).toBe(false);
  });
});

describe('pinAcceptsType - variable Get recommendations (dragging an input)', () => {
  const draggedFloatInput = pin({ direction: 'input', dataType: FLOAT64 });

  it('recommends a Float64 variable', () => {
    expect(pinAcceptsType(draggedFloatInput, FLOAT64)).toBe(true);
  });

  it('does not recommend a String variable', () => {
    expect(pinAcceptsType(draggedFloatInput, STRING)).toBe(false);
  });
});

describe('pinAcceptsType - OneOf pin converges to compatible members only', () => {
  const oneOf: DataType = { kind: 'OneOf', inner: [FLOAT64, SERIES_FLOAT64] };
  const draggedOneOfInput = pin({ direction: 'input', dataType: oneOf });

  it('matches members of the OneOf set', () => {
    expect(pinAcceptsType(draggedOneOfInput, FLOAT64)).toBe(true);
    expect(pinAcceptsType(draggedOneOfInput, SERIES_FLOAT64)).toBe(true);
  });

  it('rejects types outside the OneOf set', () => {
    expect(pinAcceptsType(draggedOneOfInput, STRING)).toBe(false);
    expect(pinAcceptsType(draggedOneOfInput, INT64)).toBe(false);
  });
});

describe('pinAcceptsType - function IO filtered by precise type', () => {
  it('a DataSeries<Float64> function input accepts only a matching output', () => {
    const draggedSeriesOutput = pin({ direction: 'output', dataType: SERIES_FLOAT64 });
    const funcInput = pin({ direction: 'input', nodeId: 'fn', dataType: SERIES_FLOAT64 });
    const funcScalarInput = pin({ direction: 'input', nodeId: 'fn', dataType: FLOAT64 });
    expect(pinAcceptsType(draggedSeriesOutput, buildPinDataType(funcInput))).toBe(true);
    expect(pinAcceptsType(draggedSeriesOutput, buildPinDataType(funcScalarInput))).toBe(false);
  });
});

describe('pinAcceptsType - Struct family matching', () => {
  const draggedModelOutput = pin({ direction: 'output', dataType: OLS_MODEL });

  it('allows a concrete model output to connect to a Model family input', () => {
    expect(pinAcceptsType(draggedModelOutput, MODEL, TYPE_SYSTEM)).toBe(true);
  });

  it('does not allow unrelated Struct outputs into a Model family input', () => {
    const draggedResultOutput = pin({ direction: 'output', dataType: OLS_RESULT });
    expect(pinAcceptsType(draggedResultOutput, MODEL, TYPE_SYSTEM)).toBe(false);
  });
});

describe('findAutoConnectPinIndex via effective call definition', () => {
  const callBase = {
    name: 'Call Function',
    category: ['Functions'],
    nodeType: CALL_FUNCTION_NODE_TYPE,
    nodeMetadata: {
      uiStyle: 'function',
      supports_dynamic_pins: true,
      graph_scope: 'any' as const,
      shell_role: null,
    },
    pinSlots: [],
    typeCapabilities: [],
  };

  it('connects exec output drag to exec input on projected call pinSlots', () => {
    const effective = resolveEffectiveDefinition(callBase, {
      subGraphId: 'fn-1',
      ...defaultFunctionSignature(),
    });
    const draggedExecOutput = pin({ direction: 'output', type: 'exec' });
    expect(findAutoConnectPinIndex(effective.pinSlots, draggedExecOutput)).toBe(0);
  });
});

describe('isPinCompatible reuses pinAcceptsType', () => {
  it('matches output -> input of the same structured type', () => {
    const out = pin({ id: 'o', nodeId: 'a', direction: 'output', dataType: SERIES_FLOAT64 });
    const inSeries = pin({ id: 'i', nodeId: 'b', direction: 'input', dataType: SERIES_FLOAT64 });
    const inScalar = pin({ id: 'i2', nodeId: 'b', direction: 'input', dataType: FLOAT64 });
    expect(isPinCompatible(inSeries, out)).toBe(true);
    expect(isPinCompatible(inScalar, out)).toBe(false);
  });

  it('rejects same-direction and same-node pairs', () => {
    const out1 = pin({ id: 'o1', nodeId: 'a', direction: 'output', dataType: FLOAT64 });
    const out2 = pin({ id: 'o2', nodeId: 'c', direction: 'output', dataType: FLOAT64 });
    const sameNodeInput = pin({ id: 'i', nodeId: 'a', direction: 'input', dataType: FLOAT64 });
    expect(isPinCompatible(out2, out1)).toBe(false); // same direction
    expect(isPinCompatible(sameNodeInput, out1)).toBe(false); // same node
  });

  it('highlights concrete Struct model outputs for Model family inputs', () => {
    const out = pin({ id: 'modelOut', nodeId: 'ols', direction: 'output', dataType: OLS_MODEL });
    const input = pin({ id: 'modelIn', nodeId: 'predict', direction: 'input', dataType: MODEL });
    expect(isPinCompatible(input, out, TYPE_SYSTEM)).toBe(true);
  });
});

describe('canConnectPins', () => {
  it('accepts compatible pins regardless of argument order', () => {
    const out = pin({ id: 'modelOut', nodeId: 'ols', direction: 'output', dataType: OLS_MODEL });
    const input = pin({ id: 'modelIn', nodeId: 'predict', direction: 'input', dataType: MODEL });

    expect(canConnectPins(out, input, TYPE_SYSTEM)).toBe(true);
    expect(canConnectPins(input, out, TYPE_SYSTEM)).toBe(true);
  });

  it('rejects same-node, same-direction, and unrelated Struct pins', () => {
    const out = pin({ id: 'modelOut', nodeId: 'ols', direction: 'output', dataType: OLS_MODEL });
    const sameNodeInput = pin({ id: 'same', nodeId: 'ols', direction: 'input', dataType: MODEL });
    const otherOutput = pin({ id: 'otherOut', nodeId: 'other', direction: 'output', dataType: OLS_MODEL });
    const resultInput = pin({ id: 'resultIn', nodeId: 'consumer', direction: 'input', dataType: OLS_RESULT });

    expect(canConnectPins(out, sameNodeInput, TYPE_SYSTEM)).toBe(false);
    expect(canConnectPins(out, otherOutput, TYPE_SYSTEM)).toBe(false);
    expect(canConnectPins(out, resultInput, TYPE_SYSTEM)).toBe(false);
  });
});
