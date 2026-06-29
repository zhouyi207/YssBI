import { describe, expect, it } from 'vitest';
import {
  derivePinConnectionView,
  derivePinLinks,
  otherEndpointFromConnectionId,
} from './pinLinks';

describe('pinLinks', () => {
  it('derives the other pin id from a connection id', () => {
    expect(otherEndpointFromConnectionId('out->in', 'out')).toBe('in');
    expect(otherEndpointFromConnectionId('out->in', 'in')).toBe('out');
  });

  it('derives runtime links from pinConnections ids', () => {
    expect(derivePinLinks('pin-a', ['pin-a->pin-b', 'pin-c->pin-a'])).toEqual([
      'pin-b',
      'pin-c',
    ]);
  });

  it('derives connected state from pinConnections ids', () => {
    expect(derivePinConnectionView(undefined)).toEqual({
      connected: false,
      linkCount: 0,
      connectionIds: [],
    });
    expect(derivePinConnectionView(['pin-a->pin-b'])).toEqual({
      connected: true,
      linkCount: 1,
      connectionIds: ['pin-a->pin-b'],
    });
  });
});
