import { describe, expect, it, vi, beforeEach } from 'vitest';
import { canCreateFunctionNodeInGraph } from './dropFunctionIntoEventGraph';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';

vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  getActiveLayoutTab: vi.fn(),
}));

const draggedFunction = {
  type: 'function' as const,
  id: 'functions/A.yssbi-function',
};

describe('canCreateFunctionNodeInGraph', () => {
  beforeEach(() => {
    vi.mocked(getActiveLayoutTab).mockReset();
  });

  it('allows a function resource in an event graph without a modifier', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'events/Main.yssbi-event',
      tab: { id: 'events/Main.yssbi-event', type: 'event', component: 'GraphEditor' },
    });

    expect(canCreateFunctionNodeInGraph('g1', draggedFunction)).toBe(true);
    expect(canCreateFunctionNodeInGraph('g1', { type: 'event', id: 'events/Main.yssbi-event' })).toBe(false);
  });

  it('allows descriptor-backed function creation in a different function graph', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'functions/B.yssbi-function',
      tab: { id: 'functions/B.yssbi-function', type: 'function', component: 'GraphEditor' },
    });

    expect(canCreateFunctionNodeInGraph('g1', draggedFunction)).toBe(true);
  });

  it('rejects dropping a function onto itself', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: draggedFunction.id,
      tab: { id: draggedFunction.id, type: 'function', component: 'GraphEditor' },
    });

    expect(canCreateFunctionNodeInGraph('g1', draggedFunction)).toBe(false);
  });

  it('rejects when active tab is not an event or function graph', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'worksheets/foo',
      tab: { id: 'worksheets/foo', type: 'worksheet', component: 'WorksheetEditor' },
    });

    expect(canCreateFunctionNodeInGraph('g1', draggedFunction)).toBe(false);
  });
});
