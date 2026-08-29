import { beforeEach, describe, expect, it, vi } from 'vitest';

import type { WorkbenchPanelInfo } from '@/features/core/dockview/workbenchRead';
import { ensureEditorViewport } from '@/features/core/viewport';
import { openEditorTab } from './openEditorTab';
import { switchEditorTab } from './switchEditorTab';
import { openGraphInEditor } from './openGraphInEditor';

const openedPanel: WorkbenchPanelInfo = {
  panelInstanceId: 'panel-returned',
  groupId: 'group-returned',
  component: 'GraphEditor',
  title: 'Main',
  metadata: {
    role: 'editor',
    resourceRef: 'events/Main.yssbi-event',
    resourceKind: 'event',
    pinned: true,
  },
  active: true,
  location: { type: 'grid' },
};

vi.mock('@/features/core/viewport', () => ({
  ensureEditorViewport: vi.fn(),
  editorViewportScope: (groupId: string, graphPath: string) => ({ groupId, graphPath }),
}));

vi.mock('./openEditorTab', () => ({
  openEditorTab: vi.fn(),
  isEditorOpenRejectionHandled: vi.fn(() => false),
}));

vi.mock('./switchEditorTab', () => ({
  switchEditorTab: vi.fn(async () => true),
}));

vi.mock('@/features/application/observability/appLogger', () => ({
  logger: { graph: { trace: vi.fn() } },
}));

describe('openGraphInEditor', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.mocked(openEditorTab).mockResolvedValue(openedPanel);
  });

  it('uses the authoritative panel and group returned by the awaited editor open', async () => {
    await expect(openGraphInEditor(
      'events/Main.yssbi-event',
      'Main',
      'event',
      'requested-group',
    )).resolves.toBe(openedPanel);

    expect(openEditorTab).toHaveBeenCalledWith({
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
      pinned: true,
    }, {
      targetGroupId: 'requested-group',
      pinned: true,
      insertIndex: undefined,
    });
    expect(ensureEditorViewport).toHaveBeenCalledWith({
      groupId: 'group-returned',
      graphPath: 'events/Main.yssbi-event',
    });
    expect(switchEditorTab).toHaveBeenCalledWith('group-returned', {
      id: 'events/Main.yssbi-event',
      type: 'event',
      component: 'GraphEditor',
      pinned: true,
    });
    expect(vi.mocked(openEditorTab).mock.invocationCallOrder[0])
      .toBeLessThan(vi.mocked(ensureEditorViewport).mock.invocationCallOrder[0]);
  });
});
