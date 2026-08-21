import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorStore } from '@/features/core/editor';
import {
  focusCanvasSelection,
  focusDetails,
  focusResultSidebar,
} from './rightSidebarActions';

const setVariablesGraphScopeFromResource = vi.hoisted(() => vi.fn());
vi.mock('@/features/core/editor/detail/variablesGraphScope', () => ({
  setVariablesGraphScopeFromResource,
}));

describe('rightSidebarActions', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    useEditorStore.setState({ detailFocus: null, rightSidebarTab: 'details' });
  });

  it('routes one exact Canvas node to Inspect', () => {
    focusCanvasSelection('events/Main.yssbi-event', ['node-1']);
    expect(useEditorStore.getState()).toMatchObject({
      rightSidebarTab: 'inspect',
      detailFocus: { kind: 'node', id: 'node-1', graphPath: 'events/Main.yssbi-event' },
    });
  });

  it('does not choose one node for multi-selection and clears stale node focus', () => {
    focusCanvasSelection('events/Main.yssbi-event', ['node-1']);
    focusCanvasSelection('events/Main.yssbi-event', ['node-1', 'node-2']);
    expect(useEditorStore.getState()).toMatchObject({
      rightSidebarTab: 'inspect',
      detailFocus: null,
    });
  });

  it('keeps non-node detail focus while an empty Canvas selection shows Inspect', () => {
    useEditorStore.setState({ detailFocus: { kind: 'variable', id: 'variable-1' } });
    focusCanvasSelection('events/Main.yssbi-event', []);
    expect(useEditorStore.getState()).toMatchObject({
      rightSidebarTab: 'inspect',
      detailFocus: { kind: 'variable', id: 'variable-1' },
    });
  });

  it('routes resources to Details and explicit results to Result', () => {
    focusDetails({ kind: 'function', path: 'functions/F.yssbi-function' });
    expect(useEditorStore.getState().detailFocus).toEqual({
      kind: 'function',
      path: 'functions/F.yssbi-function',
    });
    expect(useEditorStore.getState().rightSidebarTab).toBe('details');
    expect(setVariablesGraphScopeFromResource).toHaveBeenCalledWith('functions/F.yssbi-function');

    focusResultSidebar();
    expect(useEditorStore.getState().rightSidebarTab).toBe('result');
  });
});
