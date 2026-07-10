import { beforeEach, describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';
import { useLayoutStore } from './layoutStore';
import { enterZenMode, exitZenMode, isZenModeActive, toggleZenMode } from './workbenchZenMode';

describe('workbenchZenMode', () => {
  beforeEach(() => {
    useLayoutStore.setState({
      rootId: 'root',
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
      zenMode: false,
    });
  });

  it('hides workbench parts without persisting and restores snapshot on exit', () => {
    useLayoutStore.getState().updateNode('sidebar', { data: { ...useLayoutStore.getState().nodes.sidebar!.data, visible: true } });
    useLayoutStore.getState().updateNode('panel', { data: { ...useLayoutStore.getState().nodes.panel!.data, visible: true } });

    enterZenMode();
    expect(isZenModeActive()).toBe(true);
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(false);
    expect(useLayoutStore.getState().nodes.panel?.data?.visible).toBe(false);
    expect(useLayoutStore.getState().nodes.sidebar?.pixelSize).toBe(260);

    exitZenMode();
    expect(isZenModeActive()).toBe(false);
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(true);
    expect(useLayoutStore.getState().nodes.panel?.data?.visible).toBe(true);
  });

  it('toggleZenMode enters and exits', () => {
    toggleZenMode();
    expect(isZenModeActive()).toBe(true);
    toggleZenMode();
    expect(isZenModeActive()).toBe(false);
  });
});
