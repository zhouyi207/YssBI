import { describe, expect, it } from 'vitest';
import type { EditorConnectionProjectionDto, PortAddressDto } from '@/shared/types/dto/editorProjection';
import {
  evaluatePinViewState,
  inspectableRefsFromPinView,
  resolveUpstreamOutputs,
} from './pinViewTarget';

const graphPath = 'events/Main.yssbi-event';
const output: PortAddressDto = {
  kind: 'declared',
  nodeId: 'node-out',
  portKey: 'result',
};
const input: PortAddressDto = {
  kind: 'declared',
  nodeId: 'node-in',
  portKey: 'data',
};
const connection: EditorConnectionProjectionDto = {
  connectionId: 'connection-1',
  output,
  input,
  order: null,
};

describe('pinViewTarget', () => {
  it('builds an authoritative output-pin history ref from a structured address', () => {
    const state = evaluatePinViewState({
      graphPath,
      address: output,
      direction: 'output',
      isExec: false,
    });

    expect(state).toMatchObject({ showMenu: true, enabled: true, disabledReason: null });
    expect(state.refs).toEqual([{ kind: 'outputPin', graphPath, output }]);
  });

  it('resolves an input only to its connected upstream output address', () => {
    const state = evaluatePinViewState({
      graphPath,
      address: input,
      direction: 'input',
      isExec: false,
      connections: [connection],
    });

    expect(resolveUpstreamOutputs(input, [connection])).toEqual([output]);
    expect(state.refs).toEqual([{ kind: 'outputPin', graphPath, output }]);
  });

  it('never creates input history when the connection does not target that input', () => {
    const otherInput: PortAddressDto = { ...input, portKey: 'other' };
    expect(inspectableRefsFromPinView({
      graphPath,
      address: otherInput,
      direction: 'input',
      isExec: false,
      connections: [connection],
    })).toEqual([]);
  });

  it('hides view for exec and unconnected input pins', () => {
    expect(evaluatePinViewState({
      graphPath,
      address: input,
      direction: 'input',
      isExec: true,
      connections: [connection],
    }).showMenu).toBe(false);
    expect(evaluatePinViewState({
      graphPath,
      address: input,
      direction: 'input',
      isExec: false,
      connections: [],
    }).showMenu).toBe(false);
  });

  it('keeps output history lookup enabled before any run', () => {
    expect(evaluatePinViewState({
      graphPath,
      address: output,
      direction: 'output',
      isExec: false,
    })).toMatchObject({ showMenu: true, enabled: true, disabledReason: null });
  });
});
