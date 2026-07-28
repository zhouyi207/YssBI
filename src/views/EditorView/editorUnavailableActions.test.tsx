// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeContextMenu } from './ContextMenu/NodeContextMenu';
import { NodeCatalogTreeView } from './Layout/nodeCatalog/NodeCatalogTreeView';
import { SidebarNodeRow } from './Layout/sidebar/rows/SidebarNodeRow';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));
vi.mock('@tanstack/react-virtual', () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getTotalSize: () => count * 28,
    getVirtualItems: () => Array.from({ length: count }, (_, index) => ({
      index,
      key: index,
      start: index * 28,
    })),
  }),
}));
vi.mock('@dnd-kit/core', () => ({
  useDraggable: ({ disabled }: { disabled: boolean }) => ({
    attributes: { 'data-drag-enabled': String(!disabled) },
    listeners: {},
    setNodeRef: () => undefined,
  }),
}));

describe('unavailable node creation UI', () => {
  let host: HTMLDivElement;
  let root: Root;

  beforeEach(() => {
    host = document.createElement('div');
    document.body.appendChild(host);
    root = createRoot(host);
  });

  afterEach(() => {
    act(() => root.unmount());
    document.body.innerHTML = '';
  });

  it('disables duplicate in the node context menu', () => {
    const onDuplicate = vi.fn();
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          onCopy={vi.fn()}
          onCut={vi.fn()}
          onDuplicate={onDuplicate}
          onDelete={vi.fn()}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const duplicate = [...document.querySelectorAll('button')]
      .find((button) => button.textContent?.includes('contextMenu.node.duplicate'));
    expect((duplicate as HTMLButtonElement | undefined)?.disabled).toBe(true);
    duplicate?.click();
    expect(onDuplicate).not.toHaveBeenCalled();
  });

  it.each([
    {
      name: 'managed node',
      capabilities: { managed: true, canCopy: false, canDelete: false },
    },
    {
      name: 'copyable but non-deletable node',
      capabilities: { managed: false, canCopy: true, canDelete: false },
    },
  ])('disables delete and cut for a $name', ({ capabilities }) => {
    const onCut = vi.fn();
    const onDelete = vi.fn();
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          capabilities={capabilities}
          onCopy={vi.fn()}
          onCut={onCut}
          onDuplicate={vi.fn()}
          onDelete={onDelete}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const buttons = [...document.querySelectorAll('button')];
    const cut = buttons.find((button) => button.textContent?.includes('contextMenu.node.cut'));
    const deleteButton = buttons.find((button) => button.textContent?.includes('contextMenu.node.delete'));
    expect((cut as HTMLButtonElement | undefined)?.disabled).toBe(true);
    expect((deleteButton as HTMLButtonElement | undefined)?.disabled).toBe(true);
    cut?.click();
    deleteButton?.click();
    expect(onCut).not.toHaveBeenCalled();
    expect(onDelete).not.toHaveBeenCalled();
  });

  it('requires both copy and delete capability before enabling cut', () => {
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          capabilities={{ managed: false, canCopy: false, canDelete: true }}
          onCopy={vi.fn()}
          onCut={vi.fn()}
          onDuplicate={vi.fn()}
          onDelete={vi.fn()}
          onBreakAllLinks={vi.fn()}
          onSelectLinked={vi.fn()}
          onClose={vi.fn()}
        />,
      );
    });

    const buttons = [...document.querySelectorAll('button')];
    const cut = buttons.find((button) => button.textContent?.includes('contextMenu.node.cut'));
    const deleteButton = buttons.find((button) => button.textContent?.includes('contextMenu.node.delete'));
    expect((cut as HTMLButtonElement | undefined)?.disabled).toBe(true);
    expect((deleteButton as HTMLButtonElement | undefined)?.disabled).toBe(false);
  });

  it('renders palette leaves as disabled actions', () => {
    act(() => {
      root.render(
        <NodeCatalogTreeView
          items={[{ nodeType: 'Math:Add', title: 'Add', category: ['Math'] }]}
          onLeafClick={vi.fn()}
          leafActionsEnabled={false}
        />,
      );
    });

    const add = [...host.querySelectorAll('button')]
      .find((button) => button.textContent?.includes('Add'));
    expect((add as HTMLButtonElement | undefined)?.disabled).toBe(true);
  });

  it('keeps sidebar catalog rows non-draggable while creation is unavailable', () => {
    act(() => {
      root.render(
        <SidebarNodeRow
          item={{ nodeType: 'Math:Add', title: 'Add', category: ['Math'] }}
          level={0}
          selected={false}
          onClick={vi.fn()}
          creationEnabled={false}
        />,
      );
    });

    expect(host.querySelector('[data-drag-enabled="true"]')).toBeNull();
  });
});
