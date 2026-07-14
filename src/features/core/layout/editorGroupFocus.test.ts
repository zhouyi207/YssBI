import { describe, expect, it } from 'vitest';
import { getNextActiveEditorGroupId } from './editorGroupFocus';
import { useLayoutStore } from './layoutStore';
import { useEditorTabStore } from './editorTabStore';
import { DEFAULT_EDITOR_GROUP_ID } from './workbenchLayoutDefaults';

describe('editorGroupFocus', () => {
  it('returns MRU group excluding the closing group', () => {
    useLayoutStore.setState({
      activeEditorGroupId: 'editor_group_2',
      recentEditorGroupIds: ['editor_group_2', DEFAULT_EDITOR_GROUP_ID],
    });
    useEditorTabStore.getState().ensureGroupPlacement(DEFAULT_EDITOR_GROUP_ID);
    useEditorTabStore.getState().ensureGroupPlacement('editor_group_2');

    expect(getNextActiveEditorGroupId('editor_group_2')).toBe(DEFAULT_EDITOR_GROUP_ID);
  });
});
