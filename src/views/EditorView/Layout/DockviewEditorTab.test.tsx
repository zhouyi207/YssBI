// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { DockviewReact } from 'dockview-react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { DockviewPanelParams } from '@/features/core/dockview';

const mocks = vi.hoisted(() => ({
  closeTab: vi.fn(),
  closeOtherTabs: vi.fn(),
  closeSavedTabsInGroup: vi.fn(),
  closeAllTabsInGroup: vi.fn(),
  listDockviewGroupTabs: vi.fn(),
  isGraphResourceDirty: vi.fn(),
}));


vi.mock('@/features/application/editor/closeEditorTab', () => ({
  closeEditorTab: vi.fn(),
}));
vi.mock('@/features/application/editor/tabCommands', () => ({
  closeTab: mocks.closeTab,
  closeOtherTabs: mocks.closeOtherTabs,
  closeSavedTabsInGroup: mocks.closeSavedTabsInGroup,
  closeAllTabsInGroup: mocks.closeAllTabsInGroup,
}));
vi.mock('@/features/application/editor/dockviewTabProjection', () => ({
  listDockviewGroupTabs: mocks.listDockviewGroupTabs,
}));
vi.mock('@/features/core/resource', () => ({
  isGraphResourceDirty: mocks.isGraphResourceDirty,
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { DockviewEditorTab } from './DockviewEditorTab';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

const layoutTab = {
  id: 'events/Main.yssbi-event',
  type: 'event',
  component: 'GraphEditor',
} as const;

function TestPanel() {
  return null;
}

describe('DockviewEditorTab', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.listDockviewGroupTabs.mockReturnValue([
      layoutTab,
      {
        id: 'functions/Other.yssbi-function',
        type: 'function',
        component: 'GraphEditor',
      },
    ]);
    mocks.isGraphResourceDirty.mockReturnValue(false);
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.replaceChildren();
  });

  it('opens the project menu from the full Dockview tab surface and routes close exactly', () => {
    act(() => root.render(
      <div style={{ width: 640, height: 480 }}>
        <DockviewReact
          components={{ GraphEditor: TestPanel }}
          defaultTabComponent={DockviewEditorTab}
          onReady={({ api }) => {
            api.addPanel<DockviewPanelParams>({
              id: 'panel-a',
              component: 'GraphEditor',
              title: 'Main',
              params: {
                layoutTab: {
                  resourceRef: layoutTab.id,
                  kind: layoutTab.type,
                  data: { layoutTab },
                },
              },
            });
          }}
        />
      </div>,
    ));

    const tab = host.querySelector<HTMLElement>('.dv-tab')!;
    const event = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 31,
      clientY: 47,
    });

    act(() => tab.dispatchEvent(event));

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.listDockviewGroupTabs).toHaveBeenCalledTimes(1);
    const groupId = mocks.listDockviewGroupTabs.mock.calls[0]![0];
    const menu = document.querySelector<HTMLElement>('[role="menu"]')!;
    expect(menu.querySelectorAll('svg')).toHaveLength(4);
    expect(menu.querySelector('[data-slot="context-menu-separator"]')).not.toBeNull();
    expect(menu.textContent).toContain('tabBar.contextMenu.close');
    expect(menu.textContent).toContain('tabBar.contextMenu.closeOthers');
    expect(menu.textContent).toContain('tabBar.contextMenu.closeSaved');
    expect(menu.textContent).toContain('tabBar.contextMenu.closeAll');
    const closeItem = menu.querySelector<HTMLElement>('[role="menuitem"]');
    act(() => closeItem?.click());

    expect(mocks.closeTab).toHaveBeenCalledWith(groupId, layoutTab.id);
  });
});
