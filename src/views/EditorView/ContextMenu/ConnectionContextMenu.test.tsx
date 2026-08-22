// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { ConnectionContextMenu } from './ConnectionContextMenu';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let portal: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  portal = document.createElement('div');
  portal.id = 'portal';
  document.body.append(container, portal);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
  portal.remove();
});

describe('ConnectionContextMenu', () => {
  it('invokes one collection callback and closes after invocation', () => {
    const calls: string[] = [];
    const onBreak = vi.fn(() => calls.push('break'));
    const onClose = vi.fn(() => calls.push('close'));
    renderMenu(2, onBreak, onClose);

    const item = portal.querySelector<HTMLElement>('[role="menuitem"]')!;
    act(() => item.click());

    expect(onBreak).toHaveBeenCalledTimes(1);
    expect(onBreak).toHaveBeenCalledWith();
    expect(onClose).toHaveBeenCalledTimes(1);
    expect(calls).toEqual(['break', 'close']);
  });
});

function renderMenu(
  selectedCount: number,
  onBreak = vi.fn(),
  onClose = vi.fn(),
) {
  const render = (count: number) => act(() => {
    root.render(
      <ConnectionContextMenu
        position={{ x: 20, y: 30 }}
        selectedCount={count}
        onBreak={onBreak}
        onClose={onClose}
      />,
    );
  });
  render(selectedCount);
  return { rerender: render };
}
