import type { CSSProperties } from 'react';
import type { LayoutDirection, LayoutNode } from '@/shared/types/ui';
import { useLayoutStore } from '@/features/core/layout/layoutStore';
import { EDITOR_AREA_ID, PANEL_PART_ID } from '@/features/core/layout/workbenchLayoutDefaults';
import {
  resolveWorkbenchPartMaxSize,
  resolveWorkbenchViewport,
} from '@/features/core/layout/workbenchPanelSizing';
import {
  inferPanelPosition,
  isEditorPanelSash,
  isPanelPositionHorizontal,
  type PanelPosition,
} from '@/features/core/layout/panelPartLayout';
import {
  persistEditorGridDebounced,
  persistWorkbenchLayoutDebounced,
  togglePanelMaximized,
} from '@/features/core/layout/workbenchLayoutService';
import {
  computeFlexSplitSizes,
  isFlexSplitPair,
  panelFlexBasis,
  type FlexSplitPair,
} from '@/features/core/layout/splitView';
import {
  isEditorGridSash,
  panelStartSizeFromNode,
  resolveEditorGroupMinSize,
} from '@/features/core/layout/editorGridLayout';
import { schedulePartResizeCommit } from '@/features/core/layout/partResizeNotifier';
import type { WorkbenchPartId } from '@/features/core/layout/workbenchLayoutDefaults';
import { isZenModeActive } from '@/features/core/layout/workbenchZenMode';
import { addGlobalEventListener } from '@/shared/utils/globalEvent';

export type SashAxis = 'x' | 'y';

export const SASH_DRAG_BODY_CLASS = 'layout-sash-dragging';
export const SASH_DRAG_END_EVENT = 'layout-sash-drag-end';
export const SASH_CONTAINMENT_CLASS = 'layout-split-contain';

export interface LayoutFlexContext {
  panelMaximized?: boolean;
  panelPosition?: PanelPosition;
  maximizedEditorGroupId?: string | null;
}

export function layoutNodeFlexStyle(
  node: LayoutNode | undefined,
  context?: LayoutFlexContext,
): CSSProperties {
  if (!node || node.data?.visible === false) {
    return { flex: panelFlexBasis(0), minWidth: 0, minHeight: 0, overflow: 'hidden' };
  }

  if (node.id === EDITOR_AREA_ID && context?.panelMaximized) {
    const horizontal = isPanelPositionHorizontal(context.panelPosition ?? 'bottom');
    return horizontal
      ? { flex: panelFlexBasis(80), minWidth: 80, minHeight: 0, overflow: 'hidden' }
      : { flex: panelFlexBasis(80), minWidth: 0, minHeight: 80, overflow: 'hidden' };
  }

  if (node.id === PANEL_PART_ID && node.data?.maximized) {
    return { flex: '1 1 0px', minWidth: 0, minHeight: 0, overflow: 'hidden' };
  }

  if (node.data?.groupMaximizedHidden) {
    return { flex: panelFlexBasis(0), minWidth: 0, minHeight: 0, overflow: 'hidden' };
  }

  if (context?.maximizedEditorGroupId && node.id === context.maximizedEditorGroupId) {
    return { flex: '1 1 0px', minWidth: 0, minHeight: 0, overflow: 'hidden' };
  }

  if (node.pixelSize != null) {
    return {
      flex: panelFlexBasis(node.pixelSize),
      minWidth: 0,
      minHeight: 0,
      overflow: 'hidden',
    };
  }
  return { flex: `${node.size ?? 1} 1 0px`, minWidth: 0, minHeight: 0 };
}

export function sashAxis(orientation: LayoutDirection): SashAxis {
  return orientation === 'row' ? 'x' : 'y';
}

export function pointerCoord(e: MouseEvent, axis: SashAxis): number {
  return axis === 'x' ? e.clientX : e.clientY;
}

function elementSize(el: HTMLElement, axis: SashAxis): number {
  const rect = el.getBoundingClientRect();
  return axis === 'x' ? rect.width : rect.height;
}

function panelStartSize(node: LayoutNode | undefined, el: HTMLElement, axis: SashAxis): number {
  return panelStartSizeFromNode(node, elementSize(el, axis));
}

export type SashResizeTarget = {
  nodeId: string;
  startSize: number;
  minSize: number;
  maxSize: number;
  deltaSign: 1 | -1;
  panelPosition?: PanelPosition;
};

function panelTarget(
  node: LayoutNode,
  startSize: number,
  deltaSign: 1 | -1,
  panelPosition: PanelPosition,
): SashResizeTarget {
  return {
    nodeId: node.id,
    startSize,
    minSize: node.minSize ?? 0,
    maxSize: resolveWorkbenchPartMaxSize(node, resolveWorkbenchViewport(), panelPosition),
    deltaSign,
    panelPosition,
  };
}

export function resolveSashResizeTarget(
  _orientation: LayoutDirection,
  beforeNode: LayoutNode | undefined,
  afterNode: LayoutNode | undefined,
  beforeSize: number,
  afterSize: number,
  panelPosition: PanelPosition = 'bottom',
): SashResizeTarget | null {
  if (beforeNode?.pixelSize !== undefined) {
    return panelTarget(beforeNode, beforeSize, 1, panelPosition);
  }
  if (afterNode?.pixelSize !== undefined) {
    return panelTarget(afterNode, afterSize, -1, panelPosition);
  }
  if (!beforeNode) return null;
  return panelTarget(beforeNode, beforeSize, 1, panelPosition);
}

export function computeSashSize(target: SashResizeTarget, pointerDelta: number): number {
  const raw = target.startSize + target.deltaSign * pointerDelta;
  return Math.min(target.maxSize, Math.max(target.minSize, raw));
}

export function isSashAtLimit(target: SashResizeTarget, pointerDelta: number): boolean {
  const raw = target.startSize + target.deltaSign * pointerDelta;
  return raw <= target.minSize || raw >= target.maxSize;
}

/** VS Code: restore collapsed adjacent panel when sash drag starts. */
export function restoreAdjacentPanelVisibility(beforeNodeId: string, afterNodeId: string): void {
  if (isZenModeActive()) return;

  const { nodes, updateNode } = useLayoutStore.getState();

  for (const nodeId of [beforeNodeId, afterNodeId] as const) {
    const node = nodes[nodeId];
    if (node?.data?.visible !== false) continue;
    if (node.id === 'detail' && node.data?.userHidden === true) continue;

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

function setContainment(els: HTMLElement[], active: boolean): void {
  for (const el of els) {
    el.classList.toggle(SASH_CONTAINMENT_CLASS, active);
  }
}

export type SashDragContext = {
  orientation: LayoutDirection;
  beforeNodeId: string;
  afterNodeId: string;
  getBeforeEl: () => HTMLDivElement | null;
  getAfterEl: () => HTMLDivElement | null;
  onActiveChange?: (active: boolean) => void;
  onLimitChange?: (atLimit: boolean) => void;
};

type SingleDragSession = {
  mode: 'single';
  axis: SashAxis;
  startPointer: number;
  target: SashResizeTarget;
  targetEl: HTMLDivElement;
  containEls: HTMLDivElement[];
};

type FlexDragSession = {
  mode: 'flex-pair';
  axis: SashAxis;
  startPointer: number;
  pair: FlexSplitPair;
  minBefore: number;
  minAfter: number;
  beforeEl: HTMLDivElement;
  afterEl: HTMLDivElement;
  containEls: HTMLDivElement[];
};

type DragSession = SingleDragSession | FlexDragSession;

function isPanelSash(beforeNodeId: string, afterNodeId: string): boolean {
  return isEditorPanelSash(beforeNodeId, afterNodeId);
}

/**
 * VS Code-style sash: imperative DOM resize while dragging (no store / React churn),
 * single store commit on mouseup.
 */
export function attachSashDrag(sash: HTMLElement, ctx: SashDragContext): () => void {
  let session: DragSession | null = null;
  let latestDelta = 0;
  let rafId: number | null = null;
  let cleanupDrag: (() => void) | null = null;

  const applyPreview = (pointerDelta: number) => {
    if (!session) return;

    if (session.mode === 'flex-pair') {
      const { beforeSize, afterSize } = computeFlexSplitSizes(
        session.pair,
        pointerDelta,
        session.minBefore,
        session.minAfter,
      );
      session.beforeEl.style.flex = panelFlexBasis(beforeSize);
      session.afterEl.style.flex = panelFlexBasis(afterSize);
      return;
    }

    const size = computeSashSize(session.target, pointerDelta);
    session.targetEl.style.flex = panelFlexBasis(size);
    ctx.onLimitChange?.(isSashAtLimit(session.target, pointerDelta));
  };

  const clearPreviewStyles = () => {
    if (!session) return;
    if (session.mode === 'flex-pair') {
      session.beforeEl.style.flex = '';
      session.afterEl.style.flex = '';
    } else {
      session.targetEl.style.flex = '';
    }
    setContainment(session.containEls, false);
  };

  const endDrag = () => {
    if (!session) return;

    if (rafId !== null) {
      cancelAnimationFrame(rafId);
      rafId = null;
      applyPreview(latestDelta);
    }

    if (session.mode === 'flex-pair') {
      const { beforeSize, afterSize } = computeFlexSplitSizes(
        session.pair,
        latestDelta,
        session.minBefore,
        session.minAfter,
      );
      useLayoutStore.getState().commitFlexSplitResize(
        session.pair.beforeId,
        session.pair.afterId,
        beforeSize,
        afterSize,
      );
      persistEditorGridDebounced();
    } else {
      const finalSize = computeSashSize(session.target, latestDelta);
      if (session.target.nodeId === PANEL_PART_ID) {
        useLayoutStore.getState().resizeNode(
          session.target.nodeId,
          finalSize,
          session.target.panelPosition ?? 'bottom',
        );
      } else {
        useLayoutStore.getState().resizeNode(session.target.nodeId, finalSize);
      }
      if (
        session.target.nodeId === PANEL_PART_ID
        || session.target.nodeId === 'sidebar'
        || session.target.nodeId === 'detail'
      ) {
        if (!isZenModeActive()) {
          schedulePartResizeCommit(session.target.nodeId as WorkbenchPartId, finalSize);
          persistWorkbenchLayoutDebounced();
        }
      } else {
        persistEditorGridDebounced();
      }
    }

    requestAnimationFrame(clearPreviewStyles);

    session = null;
    latestDelta = 0;
    setSashDragCursor(false, ctx.orientation);
    ctx.onActiveChange?.(false);
    ctx.onLimitChange?.(false);
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

    restoreAdjacentPanelVisibility(ctx.beforeNodeId, ctx.afterNodeId);

    const axis = sashAxis(ctx.orientation);
    const { nodes } = useLayoutStore.getState();
    const panelPosition = inferPanelPosition(nodes);
    const beforeNode = nodes[ctx.beforeNodeId];
    const afterNode = nodes[ctx.afterNodeId];
    const beforeSize = panelStartSize(beforeNode, beforeEl, axis);
    const afterSize = panelStartSize(afterNode, afterEl, axis);
    const editorGridSash = isEditorGridSash(ctx.beforeNodeId, ctx.afterNodeId, nodes);

    if (editorGridSash || isFlexSplitPair(beforeNode, afterNode)) {
      session = {
        mode: 'flex-pair',
        axis,
        startPointer: pointerCoord(e, axis),
        pair: {
          beforeId: ctx.beforeNodeId,
          afterId: ctx.afterNodeId,
          beforeStart: beforeSize,
          afterStart: afterSize,
        },
        minBefore: editorGridSash
          ? resolveEditorGroupMinSize(beforeNode, ctx.orientation)
          : beforeNode?.minSize ?? 0,
        minAfter: editorGridSash
          ? resolveEditorGroupMinSize(afterNode, ctx.orientation)
          : afterNode?.minSize ?? 0,
        beforeEl,
        afterEl,
        containEls: [beforeEl, afterEl],
      };
    } else {
      const target = resolveSashResizeTarget(
        ctx.orientation,
        beforeNode,
        afterNode,
        beforeSize,
        afterSize,
        panelPosition,
      );
      if (!target) return;

      const targetEl = target.nodeId === ctx.beforeNodeId ? beforeEl : afterEl;
      session = {
        mode: 'single',
        axis,
        startPointer: pointerCoord(e, axis),
        target,
        targetEl,
        containEls: [beforeEl, afterEl],
      };
    }

    latestDelta = 0;
    setContainment([beforeEl, afterEl], true);
    setSashDragCursor(true, ctx.orientation);
    ctx.onActiveChange?.(true);

    cleanupDrag?.();
    const cleanupMove = addGlobalEventListener(window, 'mousemove', onMouseMove);
    const cleanupUp = addGlobalEventListener(window, 'mouseup', endDrag);
    cleanupDrag = () => {
      cleanupMove();
      cleanupUp();
    };
  };

  const onDoubleClick = (e: MouseEvent) => {
    e.preventDefault();
    e.stopPropagation();
    if (isPanelSash(ctx.beforeNodeId, ctx.afterNodeId)) {
      togglePanelMaximized();
      return;
    }

    const { nodes } = useLayoutStore.getState();
    if (!isEditorGridSash(ctx.beforeNodeId, ctx.afterNodeId, nodes)) return;

    const beforeEl = ctx.getBeforeEl();
    const afterEl = ctx.getAfterEl();
    if (!beforeEl || !afterEl) return;

    const axis = sashAxis(ctx.orientation);
    const beforeSize = panelStartSizeFromNode(
      nodes[ctx.beforeNodeId],
      elementSize(beforeEl, axis),
    );
    const afterSize = panelStartSizeFromNode(
      nodes[ctx.afterNodeId],
      elementSize(afterEl, axis),
    );
    useLayoutStore.getState().resetEditorGridSplitEqual(
      ctx.beforeNodeId,
      ctx.afterNodeId,
      beforeSize,
      afterSize,
    );
    persistEditorGridDebounced();
  };

  sash.addEventListener('mousedown', onMouseDown);
  sash.addEventListener('dblclick', onDoubleClick);

  return () => {
    sash.removeEventListener('mousedown', onMouseDown);
    sash.removeEventListener('dblclick', onDoubleClick);
    cleanupDrag?.();
    cleanupDrag = null;
    if (rafId !== null) cancelAnimationFrame(rafId);
    if (session) {
      clearPreviewStyles();
      session = null;
      setSashDragCursor(false, ctx.orientation);
      ctx.onActiveChange?.(false);
      ctx.onLimitChange?.(false);
    }
  };
}
