import { beforeEach, describe, expect, it } from 'vitest';
import { createInitialWorkbenchNodes, DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';
import { useLayoutStore } from './layoutStore';
import {
  resetWorkbenchLayout,
  showSidebarTab,
  toggleSidebarVisibility,
} from './workbenchLayoutService';
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

  it('does not replace the original snapshot when entering repeatedly', () => {
    useLayoutStore.getState().updateNode('sidebar', {
      data: { ...useLayoutStore.getState().nodes.sidebar!.data, visible: true },
    });

    enterZenMode();
    enterZenMode();
    exitZenMode();

    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(true);
  });

  it('ignores part visibility toggles while Zen is active', () => {
    enterZenMode();

    toggleSidebarVisibility();
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(false);

    exitZenMode();
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(true);
  });

  it('ignores requests to show a sidebar tab while Zen is active', () => {
    enterZenMode();

    showSidebarTab('charts');

    expect(useLayoutStore.getState().nodes.sidebar?.data?.currentTab).toBe('graphs');
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(false);
  });

  it('does not restore a stale pre-Zen snapshot after Reset Layout', () => {
    useLayoutStore.getState().updateNode('sidebar', {
      data: { ...useLayoutStore.getState().nodes.sidebar!.data, visible: false },
    });

    enterZenMode();
    resetWorkbenchLayout();
    expect(isZenModeActive()).toBe(false);
    exitZenMode();

    expect(isZenModeActive()).toBe(false);
    expect(useLayoutStore.getState().nodes.sidebar?.data?.visible).toBe(true);
    expect(useLayoutStore.getState().nodes.panel?.data?.visible).toBe(true);
    expect(useLayoutStore.getState().nodes.detail?.data?.visible).toBe(true);
  });
});
