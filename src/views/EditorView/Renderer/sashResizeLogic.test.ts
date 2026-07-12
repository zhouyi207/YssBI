import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types/ui';
import { panelFlexBasis } from '@/features/core/layout/splitView';
import { computeSashSize, layoutNodeFlexStyle, resolveSashResizeTarget } from './sashResizeLogic';

const fixed = (id: string, pixelSize: number, minSize = 0): LayoutNode => ({
  id,
  type: 'component',
  parentId: 'root',
  pixelSize,
  minSize,
  data: { component: 'Sidebar', visible: true },
});

const flex = (id: string): LayoutNode => ({
  id,
  type: 'col',
  parentId: 'root',
  size: 1,
  children: [],
});

describe('panelFlexBasis', () => {
  it('matches VS Code split-view flex shorthand', () => {
    expect(panelFlexBasis(260)).toBe('0 0 260px');
  });
});

describe('resolveSashResizeTarget', () => {
  it('prefers before panel when it has pixelSize', () => {
    const target = resolveSashResizeTarget('row', fixed('sidebar', 260, 240), flex('center'), 260, 800);
    expect(target?.nodeId).toBe('sidebar');
    expect(target?.deltaSign).toBe(1);
  });

  it('uses after panel for row detail sash', () => {
    const target = resolveSashResizeTarget('row', flex('center'), fixed('detail', 300, 240), 800, 300);
    expect(target?.nodeId).toBe('detail');
    expect(target?.deltaSign).toBe(-1);
  });

  it('uses after panel for col log panel sash', () => {
    const target = resolveSashResizeTarget('col', flex('editor'), fixed('panel', 200, 80), 600, 200);
    expect(target?.nodeId).toBe('panel');
    expect(target?.deltaSign).toBe(-1);
  });
});

describe('computeSashSize', () => {
  const detailTarget = {
    nodeId: 'detail',
    startSize: 300,
    minSize: 240,
    maxSize: Number.POSITIVE_INFINITY,
    deltaSign: -1 as const,
  };

  it('clamps to minSize', () => {
    expect(computeSashSize(detailTarget, 100)).toBe(240);
  });

  it('shrinks sidebar when dragging left', () => {
    const sidebarTarget = {
      nodeId: 'sidebar',
      startSize: 260,
      minSize: 240,
      maxSize: Number.POSITIVE_INFINITY,
      deltaSign: 1 as const,
    };
    expect(computeSashSize(sidebarTarget, -20)).toBe(240);
  });
});

describe('layoutNodeFlexStyle', () => {
  it('uses flex-basis only — no cross-axis width (log panel in col layout)', () => {
    const style = layoutNodeFlexStyle(fixed('panel', 200, 80));
    expect(style).toEqual({
      flex: '0 0 200px',
      minWidth: 0,
      minHeight: 0,
      overflow: 'hidden',
    });
    expect(style).not.toHaveProperty('width');
    expect(style).not.toHaveProperty('maxWidth');
  });

  it('collapses hidden panels', () => {
    expect(layoutNodeFlexStyle({
      ...fixed('sidebar', 260, 240),
      data: { component: 'Sidebar', visible: false },
    })).toMatchObject({
      flex: '0 0 0px',
    });
  });
});
