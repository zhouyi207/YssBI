// @vitest-environment happy-dom

import React, { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { Edge } from './Edge';

(globalThis as typeof globalThis & { IS_REACT_ACT_ENVIRONMENT: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

let container: HTMLDivElement;
let root: Root;

beforeEach(() => {
  container = document.createElement('div');
  document.body.appendChild(container);
  root = createRoot(container);
});

afterEach(() => {
  act(() => root.unmount());
  container.remove();
});

function renderEdge(props: Partial<React.ComponentProps<typeof Edge>> = {}) {
  act(() => {
    root.render(
      <svg>
        <Edge
          edgeId="edge-a"
          x1={10}
          y1={20}
          x2={110}
          y2={80}
          color="#123456"
          onPointerDown={() => {}}
          onContextMenu={() => {}}
          {...props}
        />
      </svg>,
    );
  });
}

describe('Edge interaction rendering', () => {
  it('renders one wide transparent hit path that alone accepts pointer events', () => {
    renderEdge();

    const group = container.querySelector('[data-edge-id="edge-a"]')!;
    const paths = [...group.querySelectorAll('path')];
    const hit = group.querySelector('[data-edge-hit-target="edge-a"]') as SVGPathElement;
    const visible = paths.find((path) => path !== hit)!;

    expect(hit).not.toBeNull();
    expect(hit.getAttribute('stroke')).toBe('transparent');
    expect(hit.getAttribute('stroke-width')).toBe('12');
    expect(hit.getAttribute('pointer-events')).toBe('stroke');
    expect(hit.getAttribute('d')).toBe(visible.getAttribute('d'));
    expect(paths.filter((path) => path.getAttribute('pointer-events') === 'stroke')).toEqual([hit]);
    expect(paths.filter((path) => path !== hit).every((path) => path.classList.contains('pointer-events-none'))).toBe(true);
  });

  it('exposes hover and selected styling without replacing error semantics', () => {
    renderEdge({ selected: true, isError: true });

    const group = container.querySelector('[data-edge-id="edge-a"]')!;
    const hit = group.querySelector('[data-edge-hit-target="edge-a"]')!;
    expect(group.getAttribute('data-selected')).toBe('true');
    expect(group.getAttribute('data-hovered')).toBe('false');
    expect(group.querySelector('[data-edge-selection-visual="true"]')).not.toBeNull();
    expect(group.querySelector('path[stroke="#ef4444"]')).not.toBeNull();

    act(() => hit.dispatchEvent(new PointerEvent('pointerover', { bubbles: true })));
    expect(group.getAttribute('data-hovered')).toBe('true');
    act(() => hit.dispatchEvent(new PointerEvent('pointerout', { bubbles: true })));
    expect(group.getAttribute('data-hovered')).toBe('false');
  });

  it('renders the selected halo below and wider than error semantic paths', () => {
    renderEdge({ selected: true, isError: true, isFlowActive: true, isRunning: true });

    const paths = [...container.querySelectorAll('[data-edge-id="edge-a"] > path')];
    const hit = paths.find((path) => path.hasAttribute('data-edge-hit-target'))!;
    const halo = paths.find((path) => path.hasAttribute('data-edge-selection-visual'))!;
    const semanticPaths = paths.filter((path) => path !== hit && path !== halo);
    const semanticWidths = semanticPaths.map((path) => Number(path.getAttribute('stroke-width')));

    expect(paths.indexOf(halo)).toBeLessThan(Math.min(...semanticPaths.map((path) => paths.indexOf(path))));
    expect(paths.indexOf(hit)).toBeGreaterThan(Math.max(...semanticPaths.map((path) => paths.indexOf(path))));
    expect(Number(halo.getAttribute('stroke-width'))).toBeGreaterThan(Math.max(...semanticWidths));
    expect(halo.classList.contains('pointer-events-none')).toBe(true);
    expect(semanticPaths.every((path) => path.classList.contains('pointer-events-none'))).toBe(true);
    expect(semanticPaths[semanticPaths.length - 1].getAttribute('stroke')).toBe('rgba(239, 68, 68, 0.25)');
    expect(container.querySelector('path[stroke="#ef4444"]')).not.toBeNull();
  });

  it('renders active flow semantics over the selected outer halo', () => {
    renderEdge({ selected: true, isFlowActive: true, isRunning: true });

    const paths = [...container.querySelectorAll('[data-edge-id="edge-a"] > path')];
    const halo = paths.find((path) => path.hasAttribute('data-edge-selection-visual'))!;
    const animated = paths.filter((path) => path.getAttribute('style')?.includes('animation'));

    expect(animated.length).toBeGreaterThan(0);
    expect(animated.every((path) => paths.indexOf(halo) < paths.indexOf(path))).toBe(true);
    expect(Number(halo.getAttribute('stroke-width'))).toBeGreaterThan(
      Math.max(...animated.map((path) => Number(path.getAttribute('stroke-width')))),
    );
    expect(animated.every((path) => path.classList.contains('pointer-events-none'))).toBe(true);
    expect(paths[paths.length - 1].hasAttribute('data-edge-hit-target')).toBe(true);
  });

  it('routes pointer, click, context, and double-click events only through the hit path', () => {
    const onPointerDown = vi.fn();
    const onClick = vi.fn();
    const onContextMenu = vi.fn();
    const onDoubleClick = vi.fn();
    renderEdge({ onPointerDown, onClick, onContextMenu, onDoubleClick });

    const hit = container.querySelector('[data-edge-hit-target="edge-a"]')!;
    act(() => hit.dispatchEvent(new PointerEvent('pointerdown', { bubbles: true, button: 0 })));
    act(() => hit.dispatchEvent(new MouseEvent('click', { bubbles: true, button: 0, detail: 1 })));
    act(() => hit.dispatchEvent(new MouseEvent('contextmenu', { bubbles: true, button: 2 })));
    act(() => hit.dispatchEvent(new MouseEvent('dblclick', { bubbles: true, button: 0 })));

    expect(onPointerDown).toHaveBeenCalledTimes(1);
    expect(onClick).toHaveBeenCalledTimes(1);
    expect(onContextMenu).toHaveBeenCalledTimes(1);
    expect(onDoubleClick).toHaveBeenCalledTimes(1);
    expect(container.querySelectorAll('[data-edge-hit-target]')).toHaveLength(1);
  });
});
