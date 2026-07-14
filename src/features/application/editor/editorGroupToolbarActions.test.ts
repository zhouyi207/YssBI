import { describe, expect, it } from 'vitest';
import { prepareEditorGroupToolbarActions } from './editorGroupToolbarActions';

describe('prepareEditorGroupToolbarActions', () => {
  it('active group shows split and close inline, lock in overflow', () => {
    expect(prepareEditorGroupToolbarActions({
      isGroupActive: true,
      alwaysShowEditorActions: false,
      locked: false,
    })).toEqual({
      primary: ['split-pointer', 'close-group'],
      secondary: ['toggle-lock'],
    });
  });

  it('inactive group moves actions to overflow only', () => {
    expect(prepareEditorGroupToolbarActions({
      isGroupActive: false,
      alwaysShowEditorActions: false,
      locked: false,
    })).toEqual({
      primary: [],
      secondary: ['split-right', 'split-down', 'toggle-lock', 'close-group'],
    });
  });

  it('inactive locked group shows unlock inline like VS Code', () => {
    expect(prepareEditorGroupToolbarActions({
      isGroupActive: false,
      alwaysShowEditorActions: false,
      locked: true,
    })).toEqual({
      primary: ['toggle-lock'],
      secondary: ['split-right', 'split-down', 'close-group'],
    });
  });

  it('alwaysShowEditorActions treats inactive like active', () => {
    expect(prepareEditorGroupToolbarActions({
      isGroupActive: false,
      alwaysShowEditorActions: true,
      locked: false,
    })).toEqual({
      primary: ['split-pointer', 'close-group'],
      secondary: ['toggle-lock'],
    });
  });
});
