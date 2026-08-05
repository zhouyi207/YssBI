// @vitest-environment happy-dom

import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveVariableSpawnType } from './variableDrop';

describe('resolveVariableSpawnType', () => {
  beforeEach(() => {
    Object.defineProperty(document, 'elementsFromPoint', {
      configurable: true,
      value: vi.fn(() => []),
    });
  });

  it('maps Alt to the stable variable set node ID', () => {
    expect(resolveVariableSpawnType({ altKey: true, ctrlKey: false }, 10, 20))
      .toBe('yssbi.project.variable.set');
  });

  it('maps Ctrl and pin drops to the stable variable get node ID', () => {
    expect(resolveVariableSpawnType({ altKey: false, ctrlKey: true }, 10, 20))
      .toBe('yssbi.project.variable.get');

    const pin = document.createElement('div');
    pin.dataset.pinId = 'pin-1';
    vi.mocked(document.elementsFromPoint).mockReturnValue([pin]);
    expect(resolveVariableSpawnType({ altKey: false, ctrlKey: false }, 10, 20))
      .toBe('yssbi.project.variable.get');
  });

  it('opens the choice menu without a modifier or pin target', () => {
    expect(resolveVariableSpawnType({ altKey: false, ctrlKey: false }, 10, 20)).toBe('menu');
  });
});
