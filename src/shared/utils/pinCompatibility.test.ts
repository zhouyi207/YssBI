import { describe, it, expect } from 'vitest';
import type { Pin, PinDirection } from '@/shared/types/domain/pin';
import type { DataType } from '@/shared/types/domain/dataType';
import { buildPinDataType, pinAcceptsType, isPinCompatible } from './pinCompatibility';

const FLOAT64: DataType = { kind: 'Float64' };
const STRING: DataType = { kind: 'String' };
const INT32: DataType = { kind: 'Int32' };
const SERIES_FLOAT64: DataType = { kind: 'DataSeries', inner: { kind: 'Float64' } };

function pin(partial: Partial<Pin> & { direction: PinDirection }): Pin {
  return {
    id: partial.id ?? 'p1',
    nodeId: partial.nodeId ?? 'n1',
    name: partial.name ?? 'pin',
    type: partial.type ?? 'object',
    links: partial.links ?? [],
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

  it('falls back to typeDisplay when no structured dataType is present', () => {
    const p = pin({ direction: 'output', type: 'object', typeDisplay: 'Float64' });
    expect(buildPinDataType(p)).toEqual(FLOAT64);
  });

  it('falls back to type + containerType when neither dataType nor typeDisplay exist', () => {
    const p = pin({ direction: 'output', type: 'number', containerType: 'dataseries' });
    expect(buildPinDataType(p)).toEqual(SERIES_FLOAT64);
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
    expect(pinAcceptsType(draggedOneOfInput, INT32)).toBe(false);
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
});
