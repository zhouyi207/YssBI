import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ensureEditorViewport } from '@/features/core/viewport';
import { openEditorTab } from './openEditorTab';
import { switchEditorTab } from './switchEditorTab';
import { openGraphInEditor } from './openGraphInEditor';

vi.mock('@/features/core/viewport', () => ({
  ensureEditorViewport: vi.fn(),
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));

vi.mock('@/features/core/layout/layoutTabQueries', () => ({
  resolveEditorTargetGroupId: vi.fn(() => 'editor-1'),
}));

vi.mock('./openEditorTab', () => ({
  openEditorTab: vi.fn(),
}));

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn(async () => true),
}));

vi.mock('@/utils/appLogger', () => ({
  logger: { graph: { trace: vi.fn() } },
}));

describe('openGraphInEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('seeds the target viewport before exposing the graph tab', async () => {
    await openGraphInEditor(
      'events/Main.yssbi-event',
      'Main',
      'event',
      'editor-1',
    );

    expect(ensureEditorViewport).toHaveBeenCalledWith({
      groupId: 'editor-1',
      graphPath: 'events/Main.yssbi-event',
    });
    expect(vi.mocked(ensureEditorViewport).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(openEditorTab).mock.invocationCallOrder[0]);
    expect(switchEditorTab).toHaveBeenCalledOnce();
  });
});
