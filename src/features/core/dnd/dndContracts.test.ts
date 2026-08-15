import { describe, expect, it } from 'vitest';

import { buildSidebarDragData } from '@/features/application/sidebar/buildSidebarDragData';
import {
  buildSidebarDragState,
  DRAG_TYPES,
  isGraphResourceDragPayload,
  isNodeTemplateDragData,
  isNodeTemplateDragState,
  isSidebarSpawnDrag,
  isTabDragData,
  parseCanvasDragPayload,
} from './dndContracts';

describe('parseCanvasDragPayload', () => {
  it('accepts sidebar variable spawn data only with a backend descriptor', () => {
    const descriptor = {
      kind: 'resourceBound' as const,
      nodeTypeId: 'variable.get',
      resourcePath: 'variables/v1',
      resourceRevision: 2,
      createArgs: { kind: 'variable' as const },
    };
    const payload = buildSidebarDragData('v1', 'count', 'variable', descriptor);
    expect(parseCanvasDragPayload(payload)?.type).toBe('node-template');
    expect(isNodeTemplateDragData(payload)).toBe(true);
    expect(isSidebarSpawnDrag(payload)).toBe(true);
  });

  it('accepts event graph-resource payload', () => {
    const payload = buildSidebarDragData('e1', 'Main', 'event');
    expect(isGraphResourceDragPayload(payload)).toBe(true);
    expect(isSidebarSpawnDrag(payload)).toBe(true);
  });

  it('accepts function graph-resource payload (same as event — open tab, not spawn node)', () => {
    const payload = buildSidebarDragData('functions/A.yssbi-function', 'MyFunc', 'function');
    expect(isGraphResourceDragPayload(payload)).toBe(true);
    expect(isNodeTemplateDragData(payload)).toBe(false);
    if (isGraphResourceDragPayload(payload)) {
      expect(payload.sidebarResource.type).toBe('function');
    }
  });

  it('accepts a node template carrying an exact backend descriptor', () => {
    const payload = {
      type: DRAG_TYPES.NODE_TEMPLATE,
      template: {
        title: 'Add',
        descriptor: { kind: 'static', nodeTypeId: 'math.add' },
      },
    } as const;

    expect(isNodeTemplateDragData(payload)).toBe(true);
  });

  it('accepts tab drag data', () => {
    const payload = { type: 'tab', tabId: 't1', sourceNodeId: 'g1' } as const;
    expect(isTabDragData(payload)).toBe(true);
    expect(parseCanvasDragPayload(payload)?.type).toBe('tab');
  });

  it('rejects missing and extra NodeSpawnTemplate fields', () => {
    expect(parseCanvasDragPayload({ type: 'node-template', template: {} })).toBeNull();
    expect(parseCanvasDragPayload({
      type: 'node-template',
      template: {
        descriptor: { kind: 'static', nodeTypeId: 'math.add' },
        nodeType: 'obsolete.add',
      },
    })).toBeNull();
    expect(parseCanvasDragPayload(null)).toBeNull();
  });
});

describe('buildSidebarDragState', () => {
  it('builds node-template drag state with the exact descriptor', () => {
    const descriptor = {
      kind: 'resourceBound' as const,
      nodeTypeId: 'variable.get',
      resourcePath: 'variables/v1',
      resourceRevision: 2,
      createArgs: { kind: 'variable' as const },
    };
    const payload = buildSidebarDragData('v1', 'count', 'variable', descriptor);
    expect(payload).not.toBeNull();
    const state = buildSidebarDragState(payload!, 10, 20);
    expect(isNodeTemplateDragState(state)).toBe(true);
    if (state.type === DRAG_TYPES.NODE_TEMPLATE) {
      expect(state.template.descriptor).toBe(descriptor);
      expect(state.x).toBe(10);
    }
  });

  it('builds graph-resource drag state for event and function', () => {
    for (const kind of ['event', 'function'] as const) {
      const payload = buildSidebarDragData(
        kind === 'event' ? 'events/Main.yssbi-event' : 'functions/A.yssbi-function',
        'Main',
        kind,
      );
      expect(payload).not.toBeNull();
      const state = buildSidebarDragState(payload!, 1, 2);
      expect(state.type).toBe(DRAG_TYPES.GRAPH_RESOURCE);
      expect('template' in state).toBe(false);
      if (state.type === DRAG_TYPES.GRAPH_RESOURCE) {
        expect(state.sidebarResource.type).toBe(kind);
        expect(state.sidebarResource.name).toBe('Main');
      }
    }
  });
});
