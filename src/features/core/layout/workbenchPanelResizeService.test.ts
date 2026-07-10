// @vitest-environment happy-dom

import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import {
  createInitialWorkbenchNodes,
  EDITOR_AREA_ID,
  PANEL_PART_ID,
} from './workbenchLayoutDefaults';
import { togglePanelMaximized } from './workbenchLayoutService';

describe('workbench panel resize service', () => {
  beforeEach(() => {
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 800 });
    Object.defineProperty(window, 'innerHeight', { configurable: true, value: 1_000 });
    const nodes = createInitialWorkbenchNodes();
    nodes.center!.type = 'row';
    nodes.center!.children = [PANEL_PART_ID, EDITOR_AREA_ID];
    nodes[PANEL_PART_ID]!.pixelSize = 640;
    nodes[PANEL_PART_ID]!.data = {
      ...nodes[PANEL_PART_ID]!.data,
      maximized: true,
      restoredPixelSize: 1_000,
    };
    useLayoutStore.setState({ nodes });
  });

  it('axis-clamps the restored side-panel size after the viewport shrinks', () => {
    togglePanelMaximized();

    const panel = useLayoutStore.getState().nodes[PANEL_PART_ID];
    expect(panel?.pixelSize).toBe(640);
    expect(panel?.data?.maximized).toBe(false);
    expect(panel?.data?.restoredPixelSize).toBeUndefined();
  });
});
