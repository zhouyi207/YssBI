// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const historyAvailability = vi.hoisted(() => ({
  activeTabId: null as string | null,
  canUndo: false,
  canRedo: false,
  pending: false,
}));

const draggableInputs = vi.hoisted(() => [] as Array<{ data: unknown; disabled?: boolean }>);

const catalogState = vi.hoisted(() => ({
  status: 'ready' as const,
  error: null,
  catalog: {
    items: [
      {
        nodeTypeId: 'yssbi.numeric.add.int64',
        title: 'Add',
        description: null,
        documentation: null,
        categoryId: 'math',
        iconId: 'math',
        styleId: 'default',
        aliases: [],
        technicalTerms: [],
        backendSearchText: ['add'],
        resourceNames: [],
        ports: [],
        parameters: [],
        creation: { kind: 'static' as const, nodeTypeId: 'yssbi.numeric.add.int64' },
      },
    ],
  },
  searchIndex: {
    search: () => [],
  },
  refresh: vi.fn(),
}));

vi.mock('@dnd-kit/core', () => ({
  useDraggable: (input: { data: unknown; disabled?: boolean }) => {
    draggableInputs.push(input);
    return { attributes: {}, listeners: {}, setNodeRef: vi.fn() };
  },
}));

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'canvas.nodePalette.searchPlaceholder': 'Search nodes...',
        'canvas.nodePalette.noMatches': 'No matching nodes',
        'common.loading': 'Loading...',
        'common.error': 'Error',
        'sidebar.noActiveGraph': 'No active graph open',
        'sidebar.noActiveGraphDescription': 'Open a graph to view commands',
        'common.undo': 'Undo',
        'common.redo': 'Redo',
      })[key] ?? key,
  }),
}));

vi.mock('@/features/application/editor', () => ({
  useEditorHistoryAvailability: () => historyAvailability,
}));

vi.mock('@/features/application/nodeCatalog/useLocalizedNodeCatalog', () => ({
  useLocalizedNodeCatalog: () => catalogState,
}));

import { SidebarCommandsTab } from './SidebarCommandsTab';
import { SidebarNodesTab } from './SidebarNodesTab';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('Sidebar tab-level empty states', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    draggableInputs.length = 0;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('registers backend Catalog items as draggable node templates', () => {
    act(() => root.render(<SidebarNodesTab />));

    expect(host.textContent).toContain('Add');
    expect(host.textContent).toContain('yssbi.numeric.add.int64');
    expect(host.querySelector('input[placeholder="Search nodes..."]')).not.toBeNull();
    expect(host.querySelector('button')).toBeNull();
    expect(draggableInputs).toContainEqual({
      id: 'sidebar-item-node-static:yssbi.numeric.add.int64',
      disabled: false,
      data: {
        type: 'node-template',
        template: {
          title: 'Add',
          descriptor: { kind: 'static', nodeTypeId: 'yssbi.numeric.add.int64' },
        },
      },
    });
  });

  it('uses the shared empty state when Commands has no active graph', () => {
    historyAvailability.activeTabId = null;
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain('No active graph open');
    expect(host.textContent).toContain('Open a graph to view commands');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
  });

  it('keeps command controls when an active graph exists', () => {
    historyAvailability.activeTabId = 'events/Main.yssbi-event';
    act(() => root.render(<SidebarCommandsTab />));
    expect(host.textContent).toContain('Undo');
    expect(host.textContent).toContain('Redo');
  });
});
