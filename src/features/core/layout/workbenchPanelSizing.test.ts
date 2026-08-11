import { describe, expect, it } from 'vitest';
import type { LayoutNode } from '@/shared/types/ui';
import {
  clampWorkbenchPartSize,
  resolveWorkbenchPartMaxSize,
} from './workbenchPanelSizing';

const panelNode = (): LayoutNode => ({
  id: 'panel',
  type: 'component',
  parentId: 'center',
  pixelSize: 200,
  minSize: 80,
  data: { component: 'PanelPart', visible: true },
});

describe('resolveWorkbenchPartMaxSize', () => {
  it('caps panel height at 80% of viewport when docked bottom', () => {
    expect(resolveWorkbenchPartMaxSize(panelNode(), { width: 1200, height: 1000 }, 'bottom')).toBe(800);
  });

  it('caps panel width at 80% of viewport when docked left or right', () => {
    expect(resolveWorkbenchPartMaxSize(panelNode(), { width: 1200, height: 1000 }, 'left')).toBe(960);
    expect(resolveWorkbenchPartMaxSize(panelNode(), { width: 1200, height: 1000 }, 'right')).toBe(960);
  });

  it('respects static node maxSize when lower', () => {
    const node = { ...panelNode(), maxSize: 400 };
    expect(resolveWorkbenchPartMaxSize(node, { width: 1200, height: 1000 })).toBe(400);
  });
});

describe('clampWorkbenchPartSize', () => {
  it('clamps panel size to viewport max', () => {
    expect(clampWorkbenchPartSize(panelNode(), 900, { width: 1200, height: 1000 })).toBe(800);
  });

  it('clamps panel size to minSize', () => {
    expect(clampWorkbenchPartSize(panelNode(), 40, { width: 1200, height: 1000 })).toBe(80);
  });
});
