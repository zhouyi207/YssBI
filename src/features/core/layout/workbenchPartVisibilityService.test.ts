import { beforeEach, describe, expect, it } from 'vitest';
import { useLayoutStore } from './layoutStore';
import {
  createInitialWorkbenchNodes,
  PANEL_PART_ID,
} from './workbenchLayoutDefaults';
import { setWorkbenchPartVisible } from './workbenchLayoutService';

describe('setWorkbenchPartVisible chrome edge cases', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: 'default_editor',
    });
  });

  it('marks sidebar userHidden when hidden via service and clears on show', () => {
    setWorkbenchPartVisible('sidebar', false, { userHidden: true });
    expect(useLayoutStore.getState().nodes.sidebar?.data?.userHidden).toBe(true);

    setWorkbenchPartVisible('sidebar', true);
    expect(useLayoutStore.getState().nodes.sidebar?.data?.userHidden).toBe(false);
  });

  it('clears panel maximized state when panel is hidden', () => {
    useLayoutStore.getState().updateNode(PANEL_PART_ID, {
      data: {
        ...useLayoutStore.getState().nodes[PANEL_PART_ID]!.data,
        maximized: true,
        restoredPixelSize: 220,
      },
    });

    setWorkbenchPartVisible(PANEL_PART_ID, false, { userHidden: true });

    const panel = useLayoutStore.getState().nodes[PANEL_PART_ID];
    expect(panel?.data?.visible).toBe(false);
    expect(panel?.data?.maximized).not.toBe(true);
    expect(panel?.data?.restoredPixelSize).toBeUndefined();
  });
});
