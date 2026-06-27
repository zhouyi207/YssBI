import { describe, expect, it } from 'vitest';
import { derivePinLinks, otherEndpointFromConnectionId } from './pinLinks';

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
});
