// @vitest-environment happy-dom
import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeContextMenu } from './ContextMenu/NodeContextMenu';

(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

describe('unavailable node actions', () => {
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

  it('enables duplicate only for an unmanaged copyable node', () => {
    const onDuplicate = vi.fn();
    act(() => {
      root.render(
        <NodeContextMenu
          position={{ x: 0, y: 0 }}
          capabilities={{ managed: false, canCopy: true, canDelete: false }}
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
      .find((button) => button.textContent?.includes('contextMenu.node.duplicate')) as HTMLButtonElement;
    expect(duplicate.disabled).toBe(false);
    act(() => duplicate.dispatchEvent(new MouseEvent('mousedown', { bubbles: true, button: 0 })));
    expect(onDuplicate).toHaveBeenCalledOnce();
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
});
