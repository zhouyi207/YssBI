// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import type { IDockviewPanelProps } from 'dockview-react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchPanelParams } from '@/features/core/dockview';

const mocks = vi.hoisted(() => {
  const activePanelListeners = new Set<() => void>();
  const rootApi = {
    onDidActivePanelChange: vi.fn((listener: () => void) => {
      activePanelListeners.add(listener);
      return { dispose: () => activePanelListeners.delete(listener) };
    }),
  };

  return {
    activePanelListeners,
    rootApi,
    rootDockviewProps: [] as unknown[],
    activePanel: undefined as unknown,
    hydrated: false,
    projectResourcesReady: false,
    bind: vi.fn(),
    unbind: vi.fn(),
    synchronizeActiveEditorTab: vi.fn(),
    layoutTabFromEditorMetadata: vi.fn((metadata: {
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
    })),
    groupContext: null as unknown,
    setActiveDrag: vi.fn(),
    updatePosition: vi.fn(),
  };
});

vi.mock('dockview-react', async () => {
  const React = await import('react');

  function DockviewReact(props: {
    onReady: (event: { api: typeof mocks.rootApi }) => void;
  }) {
    mocks.rootDockviewProps.push(props);
    React.useEffect(() => {
      props.onReady({ api: mocks.rootApi });
    }, [props]);
    return React.createElement('button', {
      type: 'button',
      className: 'dv-tab',
      'data-mocked-root-dockview': '',
    }, 'Root tab');
  }

  return {
    DockviewReact,
    themeDark: { name: 'dark', className: 'dockview-theme-dark' },
    themeLight: { name: 'light', className: 'dockview-theme-light' },
  };
});

vi.mock('@dnd-kit/core', async () => {
  const React = await import('react');
  const Passthrough = ({ children }: { children?: import('react').ReactNode }) =>
    React.createElement(React.Fragment, null, children);

  return {
    DndContext: Passthrough,
    DragOverlay: Passthrough,
    PointerSensor: class PointerSensor {},
    useSensor: () => ({}),
    useSensors: (...sensors: unknown[]) => sensors,
  };
});

vi.mock('@tauri-apps/api/window', () => ({
  getCurrentWindow: () => ({ label: 'main-window' }),
}));

vi.mock('@/features/application/layout/workbenchLayoutController', () => ({
  workbenchLayoutController: {
    bind: mocks.bind,
    unbind: mocks.unbind,
    get projectResourcesReady() {
      return mocks.projectResourcesReady;
    },
  },
}));

vi.mock('@/features/application/editor/switchEditorTab', () => ({
  synchronizeActiveEditorTab: mocks.synchronizeActiveEditorTab,
}));

vi.mock('@/features/application/editor/editorDragDropActions', () => ({
  executeEditorDragEnd: vi.fn(),
}));

vi.mock('@/features/core/dockview', () => ({
  workbenchDockviewPort: {
    get isHydrated() {
      return mocks.hydrated;
    },
    getActivePanel: () => mocks.activePanel,
  },
  layoutTabFromEditorMetadata: mocks.layoutTabFromEditorMetadata,
}));

vi.mock('@/features/core/dnd', () => ({
  snapTopLeftToCursor: (args: unknown) => args,
  buildSidebarDragState: vi.fn(),
  isSidebarSpawnDrag: () => false,
  parseCanvasDragPayload: () => null,
}));

vi.mock('@/features/core/editor', async () => {
  const { createContext } = await import('react');
  const GroupContext = createContext<string | null>(null);
  mocks.groupContext = GroupContext;
  return { GroupContext };
});

vi.mock('@/features/core/keyboard', () => ({
  useModifierKeyStore: {
    getState: () => ({ setModifierKeys: vi.fn() }),
  },
}));

vi.mock('@/features/core/settings/settingsStore', () => ({
  useSettingsStore: (selector: (state: { theme: { mode: 'dark' } }) => unknown) =>
    selector({ theme: { mode: 'dark' } }),
}));

vi.mock('@/features/core/sidebarDrag', () => ({
  useSidebarDragStore: (selector: (state: {
    setActiveDrag: typeof mocks.setActiveDrag;
    updatePosition: typeof mocks.updatePosition;
  }) => unknown) => selector({
    setActiveDrag: mocks.setActiveDrag,
    updatePosition: mocks.updatePosition,
  }),
}));

vi.mock('../Canvas/core/GraphEditor', async () => {
  const React = await import('react');
  return {
    GraphEditor: () => React.createElement(
      'span',
      { 'data-group-probe': '' },
      React.useContext(
        mocks.groupContext as import('react').Context<string | null>,
      ),
    ),
  };
});

vi.mock('../Worksheet/WorksheetEditor', () => ({
  WorksheetEditor: () => null,
}));
vi.mock('./Sidebar', () => ({ default: () => null }));
vi.mock('./Detail/DetailsPane', () => ({ DetailsPane: () => null }));
vi.mock('./Detail/InspectPane', () => ({ InspectPane: () => null }));
vi.mock('./result/ResultPanel', () => ({ ResultPanel: () => null }));
vi.mock('@/views/LogView/LogWorkspaceDockview', () => ({
  LogWorkspaceDockview: () => null,
}));
vi.mock('@/features/core/dockview/logsDockviewLayoutController', () => ({
  logsDockviewLayoutController: {},
}));
vi.mock('@/views/LogView/OutputPanel', () => ({ OutputPanel: () => null }));
vi.mock('../Canvas/overlays/WatermarkView', () => ({ WatermarkView: () => null }));
vi.mock('./WorkbenchDockviewTab', () => ({ WorkbenchDockviewTab: () => null }));
vi.mock('./WorkspaceDragOverlay', () => ({ WorkspaceDragOverlay: () => null }));

import { Workspace } from './Workspace';
import { WorkbenchEditorPanel } from './WorkbenchDockviewPanels';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('root workbench Dockview', () => {
  let host: HTMLDivElement;
  let root: Root | null;

  beforeEach(() => {
    vi.clearAllMocks();
    mocks.activePanelListeners.clear();
    mocks.rootDockviewProps.length = 0;
    mocks.activePanel = undefined;
    mocks.hydrated = false;
    mocks.projectResourcesReady = false;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    if (root) act(() => root?.unmount());
    host.remove();
  });

  it('mounts and binds exactly one themed root Dockview, then unbinds its live API', () => {
    act(() => root?.render(<Workspace />));

    expect(host.querySelectorAll('[data-testid="root-dockview"]')).toHaveLength(1);
    expect(host.querySelectorAll('[data-mocked-root-dockview]')).toHaveLength(1);
    expect(mocks.rootDockviewProps).toHaveLength(1);
    expect(mocks.rootDockviewProps[0]).toMatchObject({
      disableFloatingGroups: true,
      theme: { name: 'yssbi-dark' },
    });
    expect(mocks.rootDockviewProps[0]).not.toHaveProperty('popoutUrl');
    expect(mocks.bind).toHaveBeenCalledWith(mocks.rootApi, 'main-window');

    act(() => root?.unmount());
    root = null;

    expect(mocks.unbind).toHaveBeenCalledOnce();
    expect(mocks.unbind).toHaveBeenCalledWith(mocks.rootApi);
  });

  it('synchronizes only canonical editor activation after both readiness gates open', () => {
    act(() => root?.render(<Workspace />));

    mocks.activePanel = {
      panelInstanceId: 'logs-a',
      groupId: 'group-a',
      metadata: { role: 'view', viewId: 'logs' },
    };
    act(() => mocks.activePanelListeners.forEach((listener) => listener()));
    expect(mocks.synchronizeActiveEditorTab).not.toHaveBeenCalled();

    mocks.activePanel = {
      panelInstanceId: 'editor-a',
      groupId: 'group-a',
      metadata: {
        role: 'editor',
        resourceRef: 'events/Main.yssbi-event',
        resourceKind: 'event',
        pinned: true,
      },
    };
    act(() => mocks.activePanelListeners.forEach((listener) => listener()));
    expect(mocks.synchronizeActiveEditorTab).not.toHaveBeenCalled();

    mocks.hydrated = true;
    act(() => mocks.activePanelListeners.forEach((listener) => listener()));
    expect(mocks.synchronizeActiveEditorTab).not.toHaveBeenCalled();

    mocks.projectResourcesReady = true;
    act(() => mocks.activePanelListeners.forEach((listener) => listener()));
    expect(mocks.synchronizeActiveEditorTab).toHaveBeenCalledOnce();
    expect(mocks.synchronizeActiveEditorTab).toHaveBeenCalledWith('group-a', {
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
      pinned: true,
    });

    mocks.activePanel = {
      panelInstanceId: 'result-a',
      groupId: 'group-a',
      metadata: {
        role: 'result',
        resultKey: 'output:main',
        resultId: 'result-1',
      },
    };
    act(() => mocks.activePanelListeners.forEach((listener) => listener()));
    expect(mocks.synchronizeActiveEditorTab).toHaveBeenCalledOnce();
  });

  it('prevents Delete and Backspace from reaching a focused native root tab', () => {
    act(() => root?.render(<Workspace />));
    const tab = host.querySelector<HTMLElement>('.dv-tab')!;
    const reachedTarget = vi.fn();
    tab.addEventListener('keydown', reachedTarget);

    for (const key of ['Delete', 'Backspace']) {
      const event = new KeyboardEvent('keydown', {
        key,
        bubbles: true,
        cancelable: true,
      });
      act(() => tab.dispatchEvent(event));
      expect(event.defaultPrevented).toBe(true);
    }

    expect(reachedTarget).not.toHaveBeenCalled();
  });

  it('does not apply root tab keyboard suppression inside the nested Logs Dockview', () => {
    act(() => root?.render(<Workspace />));
    const rootHost = host.querySelector<HTMLElement>('[data-yssbi-root-dockview]')!;
    const logsHost = document.createElement('div');
    logsHost.dataset.yssbiLogsDockview = '';
    const nestedTab = document.createElement('button');
    nestedTab.className = 'dv-tab';
    logsHost.appendChild(nestedTab);
    rootHost.appendChild(logsHost);
    const reachedTarget = vi.fn();
    nestedTab.addEventListener('keydown', reachedTarget);

    const event = new KeyboardEvent('keydown', {
      key: 'Delete',
      bubbles: true,
      cancelable: true,
    });
    act(() => nestedTab.dispatchEvent(event));

    expect(event.defaultPrevented).toBe(false);
    expect(reachedTarget).toHaveBeenCalledOnce();
  });
});

describe('WorkbenchEditorPanel', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    vi.clearAllMocks();
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('derives the editor tab and updates GroupContext after a physical group move', () => {
    let groupId = 'group-left';
    const listeners = new Set<() => void>();
    const api = {
      get group() {
        return { id: groupId };
      },
      onDidGroupChange(listener: () => void) {
        listeners.add(listener);
        return { dispose: () => listeners.delete(listener) };
      },
    };
    const metadata = {
      role: 'editor',
      resourceRef: 'events/Main.yssbi-event',
      resourceKind: 'event',
    } as const;
    const props = {
      api,
      params: { metadata },
    } as unknown as IDockviewPanelProps<WorkbenchPanelParams>;

    act(() => root.render(<WorkbenchEditorPanel {...props} />));

    expect(mocks.layoutTabFromEditorMetadata).toHaveBeenCalledWith(metadata);
    expect(host.querySelector('[data-group-probe]')?.textContent).toBe('group-left');

    act(() => {
      groupId = 'group-right';
      listeners.forEach((listener) => listener());
    });

    expect(host.querySelector('[data-group-probe]')?.textContent).toBe('group-right');
  });
});
