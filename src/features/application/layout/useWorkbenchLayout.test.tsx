// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { useWorkbenchLayout } from './useWorkbenchLayout';
import { createInitialWorkbenchNodes } from '@/features/core/layout/workbenchLayoutDefaults';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { workbenchLayoutStorageKey } from '@/features/core/layout/workbenchLayoutMemento';

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

  it('bootstraps the active editor graph session after layout hydrate', async () => {
    await act(async () => {
      root.render(<Harness />);
    });

    expect(bootstrapEditorGraphSession).toHaveBeenCalledWith('default_editor');
  });
});
