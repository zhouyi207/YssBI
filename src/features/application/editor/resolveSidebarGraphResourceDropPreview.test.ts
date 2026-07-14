import { beforeEach, describe, expect, it, vi } from 'vitest';
import { resolveSidebarGraphResourceDropPreview } from './resolveSidebarGraphResourceDropPreview';

vi.mock('@/features/application/editor/canvasDrop', () => ({
  canDropFunctionIntoEventGraph: vi.fn(),
}));

import { canDropFunctionIntoEventGraph } from '@/features/application/editor/canvasDrop';

const rect = { top: 0, left: 0, width: 200, height: 100 };
const functionResource = {
  id: 'functions/A.yssbi-function',
  name: 'A',
  type: 'function' as const,
};

describe('resolveSidebarGraphResourceDropPreview', () => {
  beforeEach(() => {
    vi.mocked(canDropFunctionIntoEventGraph).mockReset();
  });

  it('prefers split preview over function-into-event on edge zones', () => {
    const preview = resolveSidebarGraphResourceDropPreview(
      functionResource,
      'group-event',
      { hit: { mode: 'split', edge: 'right' }, rect },
      true,
    );

    expect(preview).toEqual({
      kind: 'split',
      targetGroupId: 'group-event',
      edge: 'right',
      rect,
    });
    expect(canDropFunctionIntoEventGraph).not.toHaveBeenCalled();
  });

  it('uses function-into-event preview only in merge zone with shift', () => {
    vi.mocked(canDropFunctionIntoEventGraph).mockReturnValue(true);

    const preview = resolveSidebarGraphResourceDropPreview(
      functionResource,
      'group-event',
      { hit: { mode: 'merge' }, rect },
      true,
    );

    expect(preview).toEqual({
      kind: 'function-into-event',
      targetGroupId: 'group-event',
      rect,
      shiftHeld: true,
    });
  });

  it('falls back to merge preview when shift is not held in merge zone', () => {
    vi.mocked(canDropFunctionIntoEventGraph).mockReturnValue(false);

    const preview = resolveSidebarGraphResourceDropPreview(
      functionResource,
      'group-event',
      { hit: { mode: 'merge' }, rect },
      false,
    );

    expect(preview).toEqual({
      kind: 'merge',
      targetGroupId: 'group-event',
      rect,
      resourceName: 'A',
    });
  });
});
