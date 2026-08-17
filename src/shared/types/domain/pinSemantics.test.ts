import { describe, it, expect } from 'vitest';
import {
  isExecPin,
  pinFlowKind,
  pinTypeLabel,
  dataTypeToThemePinType,
  scalarPinInputKey,
} from './pinSemantics';

describe('pinSemantics', () => {
  it('detects exec pins by flow type only', () => {
    expect(isExecPin({ type: 'exec' })).toBe(true);
    expect(isExecPin({ type: 'object' })).toBe(false);
    expect(pinFlowKind({ type: 'exec' })).toBe('Exec');
    expect(pinFlowKind({ type: 'object' })).toBe('Data');
  });

  it('derives labels from structured dataType', () => {
    expect(
      pinTypeLabel({
        type: 'object',
        dataType: { kind: 'DataSeries', inner: { kind: 'Float64' } },
      }),
    ).toBe('DataSeries<Float64>');
  });

  it('requires structured dataType for data pin labels', () => {
    expect(pinTypeLabel({ type: 'object' })).toBe('unknown');
    expect(pinTypeLabel({ type: 'exec' })).toBe('exec');
  });

  it('maps structured data types to theme keys', () => {
    expect(
      dataTypeToThemePinType({ kind: 'DataFrame' }),
    ).toBe('dataframe');
  });

  it('maps scalar dataType kinds to pin input keys', () => {
    expect(scalarPinInputKey({ kind: 'Int64' })).toBe('Int64');
    expect(scalarPinInputKey({ kind: 'DataFrame' })).toBeNull();
  });
});
