import { describe, expect, it, vi, beforeEach } from 'vitest';
import { canDropFunctionIntoEventGraph } from './dropFunctionIntoEventGraph';
import { getActiveLayoutTab } from '@/features/core/layout/layoutTabQueries';

vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  getActiveLayoutTab: vi.fn(),
}));

const draggedFunction = {
  type: 'function' as const,
  id: 'functions/A.yssbi-function',
};

describe('canDropFunctionIntoEventGraph', () => {
  beforeEach(() => {
    vi.mocked(getActiveLayoutTab).mockReset();
  });

  it('allows a shifted function resource in an event graph when descriptor creation is available', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'events/Main.yssbi-event',
      tab: { id: 'events/Main.yssbi-event', type: 'event', component: 'GraphEditor' },
    });

    expect(canDropFunctionIntoEventGraph('g1', draggedFunction, false)).toBe(false);
    expect(canDropFunctionIntoEventGraph('g1', { type: 'event', id: 'events/Main.yssbi-event' }, true)).toBe(false);
    expect(canDropFunctionIntoEventGraph('g1', draggedFunction, true)).toBe(true);
  });

  it('allows descriptor-backed function creation in a different function graph', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'functions/B.yssbi-function',
      tab: { id: 'functions/B.yssbi-function', type: 'function', component: 'GraphEditor' },
    });

    expect(canDropFunctionIntoEventGraph('g1', draggedFunction, true)).toBe(true);
  });

  it('rejects dropping a function onto itself', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: draggedFunction.id,
      tab: { id: draggedFunction.id, type: 'function', component: 'GraphEditor' },
    });

    expect(canDropFunctionIntoEventGraph('g1', draggedFunction, true)).toBe(false);
  });

  it('rejects when active tab is not an event or function graph', () => {
    vi.mocked(getActiveLayoutTab).mockReturnValue({
      activeTabId: 'worksheets/foo',
      tab: { id: 'worksheets/foo', type: 'worksheet', component: 'WorksheetEditor' },
    });

    expect(canDropFunctionIntoEventGraph('g1', draggedFunction, true)).toBe(false);
  });
});
