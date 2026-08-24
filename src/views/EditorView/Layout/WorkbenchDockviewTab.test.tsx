// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { DockviewReact, type DockviewApi } from 'dockview-react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchPanelParams } from '@/features/core/dockview';

const mocks = vi.hoisted(() => ({
  dirty: false,
  groupPanels: [] as Array<{ panelInstanceId: string }>,
  requestCloseWorkbenchPanel: vi.fn(() => Promise.resolve(false)),
  requestCloseWorkbenchPanels: vi.fn(() => Promise.resolve(false)),
  listGroupPanels: vi.fn(),
  buildTabContextMenuSections: vi.fn(() => [{
    items: [{ id: 'document-action', label: 'document-action' }],
  }]),
}));

vi.mock('@/features/application/editor/workbenchPanelClose', () => ({
  requestCloseWorkbenchPanel: mocks.requestCloseWorkbenchPanel,
  requestCloseWorkbenchPanels: mocks.requestCloseWorkbenchPanels,
}));

vi.mock('@/features/application/editor/tabContextMenu', () => ({
  buildTabContextMenuSections: mocks.buildTabContextMenuSections,
}));

vi.mock('@/features/core/dockview', () => ({
  isWorkbenchActivityViewId: (viewId: string) => ['project', 'nodes', 'data', 'commands'].includes(viewId),
  layoutTabFromEditorMetadata: (metadata: {
    resourceRef: string;
    resourceKind: 'event' | 'function' | 'worksheet';
    pinned?: boolean;
    sticky?: boolean;
  }) => ({
    id: metadata.resourceRef,
    type: metadata.resourceKind,
    component: metadata.resourceKind === 'worksheet' ? 'WorksheetEditor' : 'GraphEditor',
    ...(metadata.pinned === undefined ? {} : { pinned: metadata.pinned }),
    ...(metadata.sticky === undefined ? {} : { sticky: metadata.sticky }),
  }),
  workbenchDockviewPort: {
    listGroupPanels: mocks.listGroupPanels,
  },
}));

vi.mock('@/features/core/resource', () => ({
  resourceKey: ({ id, kind }: { id: string; kind: string }) => `${kind}:${id}`,
  useDocumentStateStore: (selector: (state: {
    documents: Record<string, { dirty: boolean }>;
  }) => unknown) => selector({
    documents: {
      'event:events/Main.yssbi-event': { dirty: mocks.dirty },
    },
  }),
}));

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { WorkbenchDockviewTab } from './WorkbenchDockviewTab';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

function TestPanel() {
  return null;
}

function editorParams(): WorkbenchPanelParams {
  return {
    metadata: {
      role: 'editor',
      resourceRef: 'events/Main.yssbi-event',
      resourceKind: 'event',
      pinned: true,
    },
  };
}

function viewParams(): WorkbenchPanelParams {
  return { metadata: { role: 'view', viewId: 'logs' } };
}

function resultParams(): WorkbenchPanelParams {
  return {
    metadata: {
      role: 'result',
      resultKey: 'output:main',
      resultId: 'result-1',
      title: 'Distribution',
      presentation: { kind: 'inspector' },
      source: null,
    },
  };
}

describe('WorkbenchDockviewTab', () => {
  let host: HTMLDivElement;
  let root: Root;
  let api: DockviewApi | null;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.dirty = false;
    mocks.groupPanels = [];
    mocks.listGroupPanels.mockImplementation(() => mocks.groupPanels);
    api = null;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.replaceChildren();
  });

  function renderDockview(initialize: (readyApi: DockviewApi) => void): void {
    act(() => root.render(
      <div style={{ width: 640, height: 480 }}>
        <DockviewReact
          components={{
            GraphEditor: TestPanel,
            Logs: TestPanel,
            Result: TestPanel,
          }}
          defaultTabComponent={WorkbenchDockviewTab}
          onReady={({ api: readyApi }) => {
            api = readyApi;
            initialize(readyApi);
          }}
        />
      </div>,
    ));
  }

  function tabShell(panelInstanceId: string): HTMLElement {
    const content = host.querySelector<HTMLElement>(
      `[data-panel-instance-id="${panelInstanceId}"]`,
    );
    const tab = content?.closest<HTMLElement>('.dv-tab');
    if (!tab) throw new Error(`Missing tab ${panelInstanceId}`);
    return tab;
  }

  it('shows canonical editor chrome and routes close and middle-click without native removal', () => {
    mocks.dirty = true;
    renderDockview((readyApi) => {
      readyApi.addPanel<WorkbenchPanelParams>({
        id: 'editor-a',
        component: 'GraphEditor',
        title: 'Main',
        params: editorParams(),
      });
    });

    const content = host.querySelector<HTMLElement>('[data-panel-instance-id="editor-a"]')!;
    expect(content.querySelector('[data-workbench-tab-title]')?.textContent).toBe('Main');
    expect(content.querySelector('[data-workbench-tab-icon="event"]')).not.toBeNull();
    expect(content.querySelector('[data-workbench-tab-dirty]')).not.toBeNull();

    const closeButton = content.querySelector<HTMLButtonElement>('[data-workbench-tab-close]')!;
    const closeEvent = new MouseEvent('click', {
      button: 0,
      bubbles: true,
      cancelable: true,
    });
    act(() => closeButton.dispatchEvent(closeEvent));

    expect(closeEvent.defaultPrevented).toBe(true);
    expect(mocks.requestCloseWorkbenchPanel).toHaveBeenNthCalledWith(1, 'editor-a');
    expect(api?.getPanel('editor-a')).toBeDefined();

    const tab = tabShell('editor-a');
    const pointerDown = new MouseEvent('pointerdown', {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    const pointerUp = new MouseEvent('pointerup', {
      button: 1,
      bubbles: true,
      cancelable: true,
    });
    act(() => {
      tab.dispatchEvent(pointerDown);
      tab.dispatchEvent(pointerUp);
    });

    expect(pointerDown.defaultPrevented).toBe(true);
    expect(pointerUp.defaultPrevented).toBe(true);
    expect(mocks.requestCloseWorkbenchPanel).toHaveBeenNthCalledWith(2, 'editor-a');
    expect(api?.getPanel('editor-a')).toBeDefined();
  });

  it('keeps the existing editor document context menu', () => {
    renderDockview((readyApi) => {
      readyApi.addPanel<WorkbenchPanelParams>({
        id: 'editor-a',
        component: 'GraphEditor',
        title: 'Main',
        params: editorParams(),
      });
    });

    const panel = api?.getPanel('editor-a');
    const event = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 21,
      clientY: 34,
    });
    act(() => tabShell('editor-a').dispatchEvent(event));

    expect(event.defaultPrevented).toBe(true);
    expect(mocks.buildTabContextMenuSections).toHaveBeenCalledWith(
      {
        panelInstanceId: 'editor-a',
        groupId: panel?.group.id,
        tab: {
          id: 'events/Main.yssbi-event',
          type: 'event',
          component: 'GraphEditor',
          pinned: true,
        },
      },
      expect.any(Function),
    );
    expect(document.querySelector('[role="menu"]')?.textContent).toContain('document-action');
  });

  it('closes one physical mixed group through one batch request', () => {
    renderDockview((readyApi) => {
      const logs = readyApi.addPanel<WorkbenchPanelParams>({
        id: 'logs-a',
        component: 'Logs',
        title: 'Logs',
        params: viewParams(),
      });
      readyApi.addPanel<WorkbenchPanelParams>({
        id: 'result-a',
        component: 'Result',
        title: 'Distribution',
        params: resultParams(),
        position: { referencePanel: logs.id, direction: 'within' },
      });
    });
    mocks.groupPanels = [
      { panelInstanceId: 'logs-a' },
      { panelInstanceId: 'result-a' },
    ];

    const result = api?.getPanel('result-a');
    const event = new MouseEvent('contextmenu', {
      bubbles: true,
      cancelable: true,
      clientX: 55,
      clientY: 89,
    });
    act(() => tabShell('result-a').dispatchEvent(event));

    const closeGroupItem = [...document.querySelectorAll<HTMLElement>('[role="menuitem"]')]
      .find((item) => item.textContent?.includes('tabBar.closeGroup'));
    expect(closeGroupItem).toBeDefined();
    act(() => closeGroupItem?.click());

    expect(mocks.listGroupPanels).toHaveBeenCalledOnce();
    expect(mocks.listGroupPanels).toHaveBeenCalledWith(result?.group.id);
    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledOnce();
    expect(mocks.requestCloseWorkbenchPanels).toHaveBeenCalledWith([
      'logs-a',
      'result-a',
    ]);
    expect(mocks.requestCloseWorkbenchPanel).not.toHaveBeenCalled();
  });

});
