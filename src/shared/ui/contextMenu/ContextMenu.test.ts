import { describe, expect, it } from 'vitest';
import { resolveContextMenuStyle } from './ContextMenu';

describe('resolveContextMenuStyle', () => {
  it('defaults to point placement at cursor', () => {
    expect(resolveContextMenuStyle({ x: 100, y: 200 })).toEqual({
      left: 100,
      top: 200,
    });
  });

  it('below-end aligns menu right edge to anchor and drops below', () => {
    expect(resolveContextMenuStyle({
      x: 320,
      y: 48,
      placement: 'below-end',
      gap: 2,
    })).toEqual({
      left: 320,
      top: 50,
      transform: 'translateX(-100%)',
    });
  });
});
