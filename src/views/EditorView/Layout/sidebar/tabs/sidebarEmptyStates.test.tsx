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

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({
    t: (key: string) =>
      ({
        'sidebar.nodeCatalogUnavailable': 'Node catalog unavailable',
        'sidebar.nodeCatalogUnavailableDescription': 'Waiting for descriptors',
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

import { SidebarCommandsTab } from './SidebarCommandsTab';
import { SidebarNodesTab } from './SidebarNodesTab';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('Sidebar tab-level empty states', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('does not mount a scroll viewport for an unavailable node catalog', () => {
    act(() => root.render(<SidebarNodesTab />));
    expect(host.textContent).toContain('Node catalog unavailable');
    expect(host.textContent).toContain('Waiting for descriptors');
    expect(host.querySelector('.overlay-scrollbar-viewport')).toBeNull();
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
