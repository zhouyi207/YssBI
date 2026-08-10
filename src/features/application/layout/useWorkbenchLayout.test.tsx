// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useWorkbenchLayout } from './useWorkbenchLayout';
import {
  createInitialWorkbenchNodes,
  DEFAULT_EDITOR_GROUP_ID,
  EDITOR_AREA_ID,
} from '@/features/core/layout/workbenchLayoutDefaults';
import { snapshotEditorGridMemento } from '@/features/core/layout/editorGridMemento';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { workbenchLayoutStorageKey } from '@/features/core/layout/workbenchLayoutMemento';
import { useProjectIOStore } from '@/features/core/dataStore/projectIOStore';
import { LoadStatus } from '@/shared/types/ui/common';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: vi.fn(),
}));

vi.mock('@/features/application/editor/bootstrapEditorGraphSession', () => ({
  bootstrapEditorGraphSession: vi.fn(async () => true),
}));

import { bootstrapEditorGraphSession } from '@/features/application/editor/bootstrapEditorGraphSession';

function Harness(): null {
  useWorkbenchLayout();
  return null;
}

describe('useWorkbenchLayout', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
    localStorage.clear();
    vi.mocked(getCurrentWindow).mockReturnValue({ label: 'window-2' } as ReturnType<typeof getCurrentWindow>);
    useLayoutStore.setState({
      rootId: 'root',
      nodes: createInitialWorkbenchNodes(),
      activeEditorGroupId: DEFAULT_EDITOR_GROUP_ID,
    });
    useProjectIOStore.setState({
      status: LoadStatus.Idle,
      projectInstanceId: null,
    });
  });

  afterEach(async () => {
    await act(async () => root.unmount());
    host.remove();
  });

  it('hydrates the current Tauri window scope through the production hook', async () => {
    localStorage.setItem(workbenchLayoutStorageKey('main'), JSON.stringify({
      parts: { sidebar: { pixelSize: 260 } },
    }));
    localStorage.setItem(workbenchLayoutStorageKey('window-2'), JSON.stringify({
      parts: { sidebar: { pixelSize: 420 } },
    }));

    await act(async () => {
      root.render(<Harness />);
    });

    expect(useLayoutStore.getState().nodes.sidebar?.pixelSize).toBe(420);
  });

  it('does not overwrite an authoritative ready-project grid with persisted split groups', async () => {
    const persistedNodes = createInitialWorkbenchNodes();
    persistedNodes.editor_group_2 = {
      id: 'editor_group_2',
      type: 'component',
      parentId: EDITOR_AREA_ID,
      data: { component: 'GraphEditor' },
    };
    persistedNodes[EDITOR_AREA_ID].children = [DEFAULT_EDITOR_GROUP_ID, 'editor_group_2'];
    localStorage.setItem(workbenchLayoutStorageKey('window-2'), JSON.stringify({
      parts: { sidebar: { pixelSize: 420 } },
      editorGrid: snapshotEditorGridMemento(persistedNodes, 'editor_group_2'),
    }));
    useProjectIOStore.setState({
      status: LoadStatus.Ready,
      projectInstanceId: 'project-instance-current',
    });

    await act(async () => {
      root.render(<Harness />);
    });

    expect(useLayoutStore.getState().nodes.sidebar?.pixelSize).toBe(420);
    expect(useLayoutStore.getState().nodes[EDITOR_AREA_ID]?.children)
      .toEqual([DEFAULT_EDITOR_GROUP_ID]);
    expect(useLayoutStore.getState().nodes.editor_group_2).toBeUndefined();
    expect(useLayoutStore.getState().activeEditorGroupId).toBe(DEFAULT_EDITOR_GROUP_ID);
  });

  it('bootstraps the active editor graph session after layout hydrate', async () => {
    await act(async () => {
      root.render(<Harness />);
    });

    expect(bootstrapEditorGraphSession).toHaveBeenCalledWith('default_editor');
  });
});
