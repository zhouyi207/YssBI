// @vitest-environment happy-dom

import { act, createElement } from 'react';
import { createRoot } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { NodeSelectionPalette, type NodeSelectionOption } from './NodeSelectionPalette';

vi.mock('react-i18next', () => ({
  useTranslation: () => ({
    t: (key: string) => ({
      'contextMenu.node.selectNode': 'Select node',
      'contextMenu.node.noNodes': 'No nodes',
    }[key] ?? key),
  }),
}));

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean })
  .IS_REACT_ACT_ENVIRONMENT = true;

const nodes: NodeSelectionOption[] = [
  { id: 'node-a', title: 'First' },
  { id: 'node-b', title: 'Second' },
  { id: 'node-c', title: 'Third' },
];

let container: HTMLDivElement;

beforeEach(() => {
  container = document.createElement('div');
  document.body.append(container);
});

afterEach(() => {
  container.remove();
});

function renderPalette(currentNodeId: string | undefined, onSelectNode = vi.fn()) {
  const root = createRoot(container);
  act(() => {
    root.render(createElement(NodeSelectionPalette, {
      position: { x: 20, y: 30 },
      nodes,
      currentNodeId,
      onSelectNode,
      onClose: vi.fn(),
    }));
  });
  return { root, onSelectNode };
}

function activeNodeId(): string | undefined {
  return container.querySelector<HTMLElement>('[data-node-selection-option][aria-selected="true"]')
    ?.dataset.nodeId;
}

describe('NodeSelectionPalette', () => {
  it('starts at the current node and selects the next node with ArrowDown and Enter', () => {
    const { root, onSelectNode } = renderPalette('node-b');
    const palette = container.querySelector<HTMLElement>('[role="listbox"]');

    expect(activeNodeId()).toBe('node-b');
    act(() => {
      palette?.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'ArrowDown',
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(activeNodeId()).toBe('node-c');

    act(() => {
      palette?.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'Enter',
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(onSelectNode).toHaveBeenCalledWith('node-c');

    act(() => root.unmount());
  });

  it('falls back to the first node when there is no current node', () => {
    const { root, onSelectNode } = renderPalette('missing');
    const palette = container.querySelector<HTMLElement>('[role="listbox"]');

    expect(activeNodeId()).toBe('node-a');
    act(() => {
      palette?.dispatchEvent(new KeyboardEvent('keydown', {
        key: 'Enter',
        bubbles: true,
        cancelable: true,
      }));
    });
    expect(onSelectNode).toHaveBeenCalledWith('node-a');

    act(() => root.unmount());
  });
});
