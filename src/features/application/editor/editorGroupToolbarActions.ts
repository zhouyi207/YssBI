/** VS Code `prepareEditorActions` — primary inline vs secondary overflow. */
export type EditorGroupToolbarActionId =
  | 'split-pointer'
  | 'split-right'
  | 'split-down'
  | 'close-group'
  | 'toggle-lock';

export interface PreparedEditorGroupToolbar {
  primary: EditorGroupToolbarActionId[];
  secondary: EditorGroupToolbarActionId[];
}

export function prepareEditorGroupToolbarActions(options: {
  isGroupActive: boolean;
  alwaysShowEditorActions: boolean;
  locked: boolean;
}): PreparedEditorGroupToolbar {
  const { isGroupActive, alwaysShowEditorActions, locked } = options;

  if (isGroupActive || alwaysShowEditorActions) {
    return {
      primary: ['split-pointer', 'close-group'],
      secondary: ['toggle-lock'],
    };
  }

  if (locked) {
    return {
      primary: ['toggle-lock'],
      secondary: ['split-right', 'split-down', 'close-group'],
    };
  }

  return {
    primary: [],
    secondary: ['split-right', 'split-down', 'toggle-lock', 'close-group'],
  };
}
