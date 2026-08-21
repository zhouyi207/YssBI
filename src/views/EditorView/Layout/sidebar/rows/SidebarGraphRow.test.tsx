// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DRAG_TYPES } from '@/features/core/dnd';

const mocks = vi.hoisted(() => ({
  listItemProps: [] as Array<Record<string, unknown>>,
}));

vi.mock('../../sidebarUi', () => ({
  SidebarListItem: (props: Record<string, unknown>) => {
    mocks.listItemProps.push(props);
    return <div>{String(props.label)}</div>;
  },
  SidebarRowActionButton: () => null,
  SIDEBAR_ROW_ICON_SIZE: 16,
}));
vi.mock('@/components/ui/tooltip', () => ({
  Tooltip: ({ children }: { children: React.ReactNode }) => children,
  TooltipTrigger: ({ children }: { children: React.ReactNode }) => children,
  TooltipContent: ({ children }: { children: React.ReactNode }) => children,
}));
vi.mock('@/features/application/editor/openGraphInEditor', () => ({
  openGraphInEditor: vi.fn(),
}));
vi.mock('@/features/application/editor', () => ({
  focusDetails: vi.fn(),
}));
vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

import { SidebarGraphRow } from './SidebarGraphRow';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

describe('SidebarGraphRow', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    mocks.listItemProps.length = 0;
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    host.remove();
  });

  it('passes the exact Function graph-resource payload to SidebarListItem', () => {
    const functionResource = {
      id: 'functions/Revenue.yssbi-function',
      name: 'Revenue',
      type: 'function' as const,
    };

    act(() => root.render(
      <SidebarGraphRow
        id={functionResource.id}
        name={functionResource.name}
        graphType={functionResource.type}
        onContextMenu={vi.fn()}
      />,
    ));

    expect(host.textContent).toBe('Revenue');
    expect(mocks.listItemProps).toHaveLength(1);
    expect(mocks.listItemProps[0]?.dragData).toEqual({
      type: DRAG_TYPES.GRAPH_RESOURCE,
      sidebarResource: functionResource,
    });
  });
});
