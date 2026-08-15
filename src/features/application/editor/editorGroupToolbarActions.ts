/** VS Code `prepareEditorActions` — primary inline vs secondary overflow. */
export type EditorGroupToolbarActionId =
  | 'split-pointer'
  | 'split-right'
  | 'split-down'
  | 'close-group';

export interface PreparedEditorGroupToolbar {
  primary: EditorGroupToolbarActionId[];
  secondary: EditorGroupToolbarActionId[];
}

export function prepareEditorGroupToolbarActions(options: {
  isGroupActive: boolean;
  alwaysShowEditorActions: boolean;
}): PreparedEditorGroupToolbar {
  const { isGroupActive, alwaysShowEditorActions } = options;

  if (isGroupActive || alwaysShowEditorActions) {
    return {
      primary: ['split-pointer', 'close-group'],
      secondary: [],
    };
  }

  return {
    primary: [],
    secondary: ['split-right', 'split-down', 'close-group'],
  };
}
