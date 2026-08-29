import { beforeEach, describe, expect, it, vi } from 'vitest';

import { useProjectIOStore } from '@/features/application/project/projectIOStore';
import { ensureEditorViewport, editorViewportScope } from '@/features/core/viewport';
import {
  synchronizeVisibleGraphPanel,
  synchronizeVisibleGraphPanels,
} from './synchronizeVisibleGraphPanel';

const dockviewMocks = vi.hoisted(() => ({
  listPanels: vi.fn(),
}));

vi.mock('@/features/core/dockview/workbenchRead', () => ({
  workbenchDockviewRead: dockviewMocks,
}));

vi.mock('@/features/core/viewport', () => ({
  ensureEditorViewport: vi.fn(),
  editorViewportScope: vi.fn((groupId: string, graphPath: string) => ({
    groupId,
    graphPath,
  })),
}));

describe('synchronizeVisibleGraphPanel', () => {
  const graphPath = 'events/Main.yssbi-event';

  beforeEach(() => {
    vi.clearAllMocks();
    dockviewMocks.listPanels.mockReturnValue([]);
  });

  it('seeds the viewport before loading the visible graph projection', async () => {
    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });

    await expect(synchronizeVisibleGraphPanel({
      groupId: 'group-preview',
      graphPath,
    })).resolves.toBe(true);

    expect(editorViewportScope).toHaveBeenCalledWith('group-preview', graphPath);
    expect(ensureEditorViewport).toHaveBeenCalledWith({
      groupId: 'group-preview',
      graphPath,
    });
    expect(loadGraph).toHaveBeenCalledWith(graphPath);
    expect(vi.mocked(ensureEditorViewport).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(loadGraph).mock.invocationCallOrder[0]);
  });

  it('seeds every visible group but loads a shared graph only once', async () => {
    const loadGraph = vi.fn(async () => true);
    useProjectIOStore.setState({ loadGraph });
    dockviewMocks.listPanels.mockReturnValue([
      {
        groupId: 'group-a',
        visible: true,
        metadata: { role: 'editor', resourceRef: graphPath, resourceKind: 'event' },
      },
      {
        groupId: 'group-b',
        visible: true,
        metadata: { role: 'editor', resourceRef: graphPath, resourceKind: 'event' },
      },
      {
        groupId: 'group-hidden',
        visible: false,
        metadata: { role: 'editor', resourceRef: 'functions/Hidden.yssbi-function', resourceKind: 'function' },
      },
    ]);

    await expect(synchronizeVisibleGraphPanels()).resolves.toBeUndefined();

    expect(ensureEditorViewport).toHaveBeenCalledTimes(2);
    expect(ensureEditorViewport).toHaveBeenNthCalledWith(1, {
      groupId: 'group-a',
      graphPath,
    });
    expect(ensureEditorViewport).toHaveBeenNthCalledWith(2, {
      groupId: 'group-b',
      graphPath,
    });
    expect(loadGraph).toHaveBeenCalledOnce();
    expect(loadGraph).toHaveBeenCalledWith(graphPath);
  });
});
