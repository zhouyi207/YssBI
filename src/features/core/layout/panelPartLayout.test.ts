import { describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes } from './workbenchLayoutDefaults';
import {
  applyPanelPosition,
} from './workbenchLayoutService';
import {
  centerLayoutForPanelPosition,
  inferPanelPosition,
  isEditorPanelSash,
  normalizePanelPosition,
} from './panelPartLayout';
import { useLayoutStore } from './layoutStore';

describe('panelPartLayout', () => {
  it('normalizes settings values', () => {
    expect(normalizePanelPosition('Left')).toBe('left');
    expect(normalizePanelPosition('Right')).toBe('right');
    expect(normalizePanelPosition('Bottom')).toBe('bottom');
    expect(normalizePanelPosition(undefined)).toBe('bottom');
  });

  it('infers bottom from default center col layout', () => {
    const nodes = createInitialWorkbenchNodes();
    expect(inferPanelPosition(nodes)).toBe('bottom');
    expect(centerLayoutForPanelPosition('bottom')).toEqual({
      type: 'col',
      children: ['editor_area', 'panel'],
    });
  });

  it('applyPanelPosition restructures center for left and right', () => {
    useLayoutStore.getState().resetWorkbenchLayout();

    applyPanelPosition('left');
    let nodes = useLayoutStore.getState().nodes;
    expect(nodes.center?.type).toBe('row');
    expect(nodes.center?.children).toEqual(['panel', 'editor_area']);
    expect(inferPanelPosition(nodes)).toBe('left');

    applyPanelPosition('right');
    nodes = useLayoutStore.getState().nodes;
    expect(nodes.center?.children).toEqual(['editor_area', 'panel']);
    expect(inferPanelPosition(nodes)).toBe('right');

    applyPanelPosition('bottom');
    nodes = useLayoutStore.getState().nodes;
    expect(nodes.center?.type).toBe('col');
    expect(nodes.center?.children).toEqual(['editor_area', 'panel']);
  });

  it('detects editor↔panel sash regardless of orientation', () => {
    expect(isEditorPanelSash('editor_area', 'panel')).toBe(true);
    expect(isEditorPanelSash('panel', 'editor_area')).toBe(true);
    expect(isEditorPanelSash('sidebar', 'center')).toBe(false);
  });
});
