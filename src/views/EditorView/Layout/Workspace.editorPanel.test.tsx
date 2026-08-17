// @vitest-environment happy-dom

import { act, useContext } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import type { IDockviewPanelProps } from 'dockview-react';
import { afterAll, afterEach, beforeAll, beforeEach, describe, expect, it, vi } from 'vitest';

import type { DockviewPanelParams } from '@/features/core/dockview';
import { GroupContext } from '@/features/core/editor';
import { DockviewEditorPanel } from './Workspace';

const mocks = vi.hoisted(() => ({
  getView: vi.fn(),
}));

vi.mock('../Renderer/viewRegistry', () => ({
  viewRegistry: { get: mocks.getView },
}));

function GroupScopeProbe() {
  return <span>{useContext(GroupContext)}</span>;
}

describe('DockviewEditorPanel', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;
  });

  afterAll(() => {
    (globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = false;
  });

  beforeEach(() => {
    mocks.getView.mockReturnValue(GroupScopeProbe);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
    vi.clearAllMocks();
  });

  it('updates the editor group scope when Dockview moves the panel', () => {
    let groupId = 'group-left';
    const listeners = new Set<() => void>();
    const api = {
      component: 'TestEditor',
      get group() {
        return { id: groupId };
      },
      onDidGroupChange(listener: () => void) {
        listeners.add(listener);
        return { dispose: () => listeners.delete(listener) };
      },
    };
    const props = { api } as unknown as IDockviewPanelProps<DockviewPanelParams>;

    act(() => root.render(<DockviewEditorPanel {...props} />));
    expect(host.textContent).toBe('group-left');

    act(() => {
      groupId = 'group-right';
      listeners.forEach((listener) => listener());
    });

    expect(host.textContent).toBe('group-right');
  });
});
