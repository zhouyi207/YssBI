import { beforeEach, describe, expect, it, vi } from 'vitest';
import { useEditorTabStore } from '@/features/core/layout/editorTabStore';
import { buildWorksheetLayoutTab } from '@/features/core/layout/layoutTabModel';
import { useWorksheetStore } from '@/features/core/worksheet/worksheetStore';
import {
  isResourceDocumentDirty,
  markResourceDirty,
  useDocumentStateStore,
  useResourceStore,
} from '@/features/core/resource';
import { uiStore } from '@/features/core/ui/UIStore';
import { saveAllDirtyGraphs } from './saveAllDirtyGraphs';

const worksheetId = 'worksheet-1';

describe('saveAllDirtyGraphs worksheet lifecycle', () => {
  beforeEach(() => {
    vi.restoreAllMocks();
    useEditorTabStore.setState({ registry: {}, placements: {} });
    useDocumentStateStore.getState().clear();
    useResourceStore.getState().clear();
    useEditorTabStore.getState().initGroupPlacement(
      'editor',
      [buildWorksheetLayoutTab(worksheetId)],
      worksheetId,
    );
    markResourceDirty({ id: worksheetId, kind: 'worksheet' }, true);
  });

  it('keeps a worksheet dirty and reports incomplete when its save basis becomes stale', async () => {
    vi.spyOn(useWorksheetStore.getState(), 'saveDocument').mockResolvedValue(false);
    const toast = vi.spyOn(uiStore, 'showToast');

    await expect(saveAllDirtyGraphs()).resolves.toBe(false);

    expect(isResourceDocumentDirty({ id: worksheetId, kind: 'worksheet' })).toBe(true);
    expect(toast).not.toHaveBeenCalled();
  });
});
