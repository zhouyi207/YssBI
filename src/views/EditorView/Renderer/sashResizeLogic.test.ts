import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types/ui';
import { computeSashSize, resolveSashResizeTarget } from './sashResizeLogic';

const fixed = (id: string, pixelSize: number, minSize = 0): LayoutNode => ({
  id,
  type: 'component',
  parentId: 'root',
  pixelSize,
  minSize,
  data: { component: 'Detail', visible: true },
});

const flex = (id: string): LayoutNode => ({
  id,
  type: 'col',
  parentId: 'root',
  size: 1,
  children: [],
});

describe('resolveSashResizeTarget', () => {
  it('prefers before panel when it has pixelSize', () => {
    const target = resolveSashResizeTarget('row', fixed('sidebar', 260), flex('center'), 260, 800);
    expect(target?.nodeId).toBe('sidebar');
    expect(target?.deltaSign).toBe(1);
  });

  it('uses after panel for row detail sash', () => {
    const target = resolveSashResizeTarget('row', flex('center'), fixed('detail', 300), 800, 300);
    expect(target?.nodeId).toBe('detail');
    expect(target?.deltaSign).toBe(-1);
  });

  it('uses after panel for col log panel sash (drag up grows the bottom panel)', () => {
    const target = resolveSashResizeTarget('col', flex('editor'), fixed('panel', 200), 600, 200);
    expect(target?.nodeId).toBe('panel');
    // after 节点在下方：向上拖（delta 为负）应增大其高度，故 deltaSign 为 -1。
    expect(target?.deltaSign).toBe(-1);
  });
});

describe('computeSashSize', () => {
  const detailTarget = {
    nodeId: 'detail',
    startSize: 300,
    minSize: 240,
    deltaSign: -1 as const,
  };

  it('clamps to minSize', () => {
    expect(computeSashSize(detailTarget, 100)).toBe(240);
  });

  it('grows detail when dragging left in row layout', () => {
    expect(computeSashSize(detailTarget, -40)).toBe(340);
  });
});
