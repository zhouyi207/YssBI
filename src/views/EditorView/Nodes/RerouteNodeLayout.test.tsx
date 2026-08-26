// @vitest-environment happy-dom

import { act } from 'react';
import { createRoot, type Root } from 'react-dom/client';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { TooltipProvider } from '@/components/ui/tooltip';
import type { PortKindDto } from '@/shared/types/dto/editorProjection';
import type { PinView } from '@/shared/types/store/graph';
import type { UINode } from '@/shared/types/ui';
import { Node } from './Node';

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
      expect(layout.dataset.rerouteKind).toBe(kind);
      expect(pins.map((pin) => pin.dataset.pinId)).toEqual([
        `${node.id}:input`,
        `${node.id}:output`,
      ]);
      expect(container.textContent).not.toContain(node.title);
      expect(container.textContent).not.toContain('Hidden');
      expect(container.querySelector('input')).toBeNull();

      const inputAnchor = pins[0].querySelector<HTMLElement>('[data-pin-connection-anchor]');
      expect(inputAnchor).not.toBeNull();
      act(() => inputAnchor!.dispatchEvent(new PointerEvent('pointerdown', {
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

});
