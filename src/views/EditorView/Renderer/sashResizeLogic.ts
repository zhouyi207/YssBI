import type { LayoutDirection, LayoutNode } from '@/shared/types/ui';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

export type SashAxis = 'x' | 'y';

export const SASH_DRAG_BODY_CLASS = 'layout-sash-dragging';
export const SASH_DRAG_END_EVENT = 'layout-sash-drag-end';

export function sashAxis(orientation: LayoutDirection): SashAxis {
  return orientation === 'row' ? 'x' : 'y';
}

export function pointerCoord(e: MouseEvent, axis: SashAxis): number {
  return axis === 'x' ? e.clientX : e.clientY;
}

export function elementSize(el: HTMLElement, axis: SashAxis): number {
  const rect = el.getBoundingClientRect();
  return axis === 'x' ? rect.width : rect.height;
}

export function flexBasis(px: number): string {
  return `0 0 ${px}px`;
}

/** Flex style for layout child wrappers — shared with sash drag preview. */
export function layoutNodeFlexStyle(node: LayoutNode | undefined): { flex: string } {
  if (!node || node.data?.visible === false) {
    return { flex: flexBasis(0) };
  }
  if (node.pixelSize !== undefined) {
    return { flex: flexBasis(node.pixelSize) };
  }
  return { flex: `${node.size ?? 1} 1 0px` };
}

export type SashResizeTarget = {
  nodeId: string;
  startSize: number;
  minSize: number;
  /** newSize = startSize + deltaSign * pointerDelta */
  deltaSign: 1 | -1;
};

function panelTarget(
  node: LayoutNode,
  startSize: number,
  deltaSign: 1 | -1,
): SashResizeTarget {
  return {
    nodeId: node.id,
    startSize,
    minSize: node.minSize ?? 0,
    deltaSign,
  };
}

export function resolveSashResizeTarget(
  orientation: LayoutDirection,
  beforeNode: LayoutNode | undefined,
  afterNode: LayoutNode | undefined,
  beforeSize: number,
  afterSize: number,
): SashResizeTarget | null {
  if (beforeNode?.pixelSize !== undefined) {
    return panelTarget(beforeNode, beforeSize, 1);
  }
  if (afterNode?.pixelSize !== undefined) {
    return panelTarget(afterNode, afterSize, orientation === 'row' ? -1 : 1);
  }
  if (!beforeNode) return null;
  return panelTarget(beforeNode, beforeSize, 1);
}

export function computeSashSize(target: SashResizeTarget, pointerDelta: number): number {
  return Math.max(target.minSize, target.startSize + target.deltaSign * pointerDelta);
}

export function restoreAdjacentPanelVisibility(beforeNodeId: string, afterNodeId: string): void {
  const { nodes, updateNode } = useLayoutStore.getState();

  for (const nodeId of [beforeNodeId, afterNodeId] as const) {
    const node = nodes[nodeId];
    if (node?.data?.visible !== false) continue;

    const restored = { ...node.data, visible: true as const };
    if (!restored.currentTab && restored.component === 'Sidebar') {
      restored.currentTab = 'graphs';
    }
    updateNode(nodeId, { data: restored });
  }
}

function setSashDragCursor(active: boolean, orientation: LayoutDirection): void {
  document.body.classList.toggle(SASH_DRAG_BODY_CLASS, active);
  document.body.classList.toggle(orientation === 'row' ? 'col-resize' : 'row-resize', active);
  if (!active) {
    document.body.classList.remove('col-resize', 'row-resize');
  }
}

export type SashDragContext = {
  orientation: LayoutDirection;
  beforeNodeId: string;
  afterNodeId: string;
  getBeforeEl: () => HTMLDivElement | null;
  getAfterEl: () => HTMLDivElement | null;
  onActiveChange?: (active: boolean) => void;
};

type DragSession = {
  axis: SashAxis;
  startPointer: number;
  target: SashResizeTarget;
  targetEl: HTMLDivElement;
};

/** Imperative sash drag — returns cleanup for mousedown listener + in-flight drag. */
export function attachSashDrag(sash: HTMLElement, ctx: SashDragContext): () => void {
  let session: DragSession | null = null;
  let latestDelta = 0;
  let rafId: number | null = null;
  let cleanupDrag: (() => void) | null = null;

  const applyPreview = (pointerDelta: number) => {
    if (!session) return;
    session.targetEl.style.flex = flexBasis(computeSashSize(session.target, pointerDelta));
  };

  const endDrag = () => {
    if (!session) return;

    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
      applyPreview(latestDelta);
    }

    useLayoutStore.getState().resizeNode(
      session.target.nodeId,
      computeSashSize(session.target, latestDelta),
    );

    session.targetEl.style.flex = '';
    session = null;

    setSashDragCursor(false, ctx.orientation);
    ctx.onActiveChange?.(false);
    window.dispatchEvent(new CustomEvent(SASH_DRAG_END_EVENT));

    cleanupDrag?.();
    cleanupDrag = null;
  };

  const onMouseMove = (e: MouseEvent) => {
    if (!session) return;
    latestDelta = pointerCoord(e, session.axis) - session.startPointer;
    if (rafId !== null) return;
    rafId = requestAnimationFrame(() => {
      rafId = null;
      applyPreview(latestDelta);
    });
  };

  const onMouseDown = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();

    const beforeEl = ctx.getBeforeEl();
    const afterEl = ctx.getAfterEl();
    if (!beforeEl || !afterEl) return;

    const axis = sashAxis(ctx.orientation);
    const { nodes } = useLayoutStore.getState();
    const target = resolveSashResizeTarget(
      ctx.orientation,
      nodes[ctx.beforeNodeId],
      nodes[ctx.afterNodeId],
      elementSize(beforeEl, axis),
      elementSize(afterEl, axis),
    );
    if (!target) return;

    const targetEl = target.nodeId === ctx.beforeNodeId ? beforeEl : afterEl;
    session = { axis, startPointer: pointerCoord(e, axis), target, targetEl };
    latestDelta = 0;

    setSashDragCursor(true, ctx.orientation);
    ctx.onActiveChange?.(true);
    restoreAdjacentPanelVisibility(ctx.beforeNodeId, ctx.afterNodeId);

    cleanupDrag?.();
    const cleanupMove = addGlobalEventListener(window, 'mousemove', onMouseMove);
    const cleanupUp = addGlobalEventListener(window, 'mouseup', endDrag);
    cleanupDrag = () => {
      cleanupMove();
      cleanupUp();
    };
  };

  sash.addEventListener('mousedown', onMouseDown);

  return () => {
    sash.removeEventListener('mousedown', onMouseDown);
    cleanupDrag?.();
    cleanupDrag = null;
    if (rafId !== null) cancelAnimationFrame(rafId);
    if (session) {
      session.targetEl.style.flex = '';
      session = null;
      setSashDragCursor(false, ctx.orientation);
      ctx.onActiveChange?.(false);
    }
  };
}
