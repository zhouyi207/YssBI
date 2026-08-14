// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { PortKindDto } from '@/shared/types/dto/editorProjection';
import type { PinView } from '@/shared/types/store/graph';
import type { UINode } from '@/shared/types/ui';
import { Node } from './Node';
import {
  REROUTE_GRIP_SIZE_PX,
  REROUTE_NODE_HEIGHT_PX,
  REROUTE_NODE_WIDTH_PX,
} from './RerouteNodeLayout';

vi.mock('react-i18next', async (importOriginal) => ({
  ...(await importOriginal<typeof import('react-i18next')>()),
  useTranslation: () => ({ t: (key: string) => key }),
}));

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
  document.querySelector('[data-yssbi-overlay-root]')?.remove();
});

function projectedPin(
  nodeId: string,
  kind: PortKindDto,
  direction: 'input' | 'output',
): PinView {
  const id = `${nodeId}:${direction}`;
  return {
    id,
    nodeId,
    name: direction === 'input' ? 'Input' : 'Output',
    type: kind === 'data' ? 'object' : 'exec',
    direction,
    kind,
    address: { kind: 'declared', nodeId, portKey: direction },
    resolvedType: kind === 'data'
      ? { display: 'Float64', resolved: true, dataType: { kind: 'Float64' } }
      : { display: 'Unknown', resolved: false, dataType: null },
    connected: true,
    linkCount: 1,
    connectionIds: [`${nodeId}:connection`],
  };
}

function projectedReroute(kind: PortKindDto): UINode {
  const id = `reroute-${kind}`;
  return {
    id,
    nodeType: `opaque.test.${kind}`,
    category: ['Hidden'],
    title: `Forbidden ${kind} title`,
    description: `Forbidden ${kind} description`,
    uiStyle: 'builtin.reroute',
    position: { x: 135, y: 246 },
    inputs: [projectedPin(id, kind, 'input')],
    outputs: [projectedPin(id, kind, 'output')],
  };
}

function renderNode(node: UINode, onPinPointerDown = vi.fn(), onPointerDown = vi.fn()) {
  act(() => root.render(
    <TooltipProvider>
      <Node
        id={node.id}
        node={node}
        selected
        onPointerDown={onPointerDown}
        onPinPointerDown={onPinPointerDown}
      />
    </TooltipProvider>,
  ));
  return { onPinPointerDown, onPointerDown };
}

describe('RerouteNodeLayout', () => {
  it.each(['data', 'control', 'effect'] as const)(
    'renders one connectable input/output with projected %s semantics and no ordinary UI',
    (kind) => {
      const node = projectedReroute(kind);
      const { onPinPointerDown } = renderNode(node);
      const nodeRoot = container.querySelector(`[data-node-id="${node.id}"]`) as HTMLDivElement;
      const layout = container.querySelector('[data-reroute-layout]') as HTMLDivElement;
      const pins = [...container.querySelectorAll('[data-pin-id]')] as HTMLDivElement[];

      expect(nodeRoot).not.toBeNull();
      expect(nodeRoot.style.transform).toBe('translate3d(135px, 246px, 0)');
      expect(nodeRoot.className).toContain('ring-2');
      expect(layout.dataset.rerouteKind).toBe(kind);
      expect(pins.map((pin) => pin.dataset.pinId)).toEqual([
        `${node.id}:input`,
        `${node.id}:output`,
      ]);
      expect(container.textContent).not.toContain(node.title);
      expect(container.textContent).not.toContain('Hidden');
      expect(container.querySelector('input')).toBeNull();
      const grip = container.querySelector('[data-reroute-grip]') as HTMLDivElement;
      expect(grip).not.toBeNull();
      expect(grip.className).toContain(kind === 'data' ? 'rounded-full' : 'rounded-none');
      expect(grip.className.includes('rotate-45')).toBe(kind === 'effect');

      act(() => pins[0].dispatchEvent(new PointerEvent('pointerdown', {
        bubbles: true,
        cancelable: true,
        button: 0,
      })));
      expect(onPinPointerDown).toHaveBeenCalledOnce();
      expect(onPinPointerDown.mock.calls[0][1]).toMatchObject({
        id: `${node.id}:input`,
        kind,
        address: { kind: 'declared', nodeId: node.id, portKey: 'input' },
      });
    },
  );

  it('locks exact compact dimensions and center grip size', () => {
    renderNode(projectedReroute('data'));
    const nodeRoot = container.querySelector('[data-node-id]') as HTMLDivElement;
    const grip = container.querySelector('[data-reroute-grip]') as HTMLDivElement;

    expect(REROUTE_NODE_WIDTH_PX).toBe(32);
    expect(REROUTE_NODE_HEIGHT_PX).toBe(20);
    expect(REROUTE_GRIP_SIZE_PX).toBe(8);
    expect(nodeRoot.style.width).toBe('32px');
    expect(nodeRoot.style.height).toBe('20px');
    expect(nodeRoot.style.minWidth).toBe('32px');
    expect(nodeRoot.style.minHeight).toBe('20px');
    expect(grip.style.width).toBe('8px');
    expect(grip.style.height).toBe('8px');
  });
});
