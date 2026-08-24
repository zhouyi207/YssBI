import { useSettingsStore } from '@/features/core/settings/settingsStore';
import type { EditorSettings } from '@/shared/types/settings';

export type OpenSideBySideDirection = NonNullable<EditorSettings['openSideBySideDirection']>;
export type EditorSplitSizingMode = NonNullable<EditorSettings['splitSizing']>;

/** VS Code `IEditorPartOptions` subset used by editor groups. */
export interface EditorPartOptions {
  openSideBySideDirection: OpenSideBySideDirection;
  splitOnDragAndDrop: boolean;
  alwaysShowEditorActions: boolean;
  closeEmptyGroups: boolean;
  splitSizing: EditorSplitSizingMode;
}

export const DEFAULT_EDITOR_PART_OPTIONS: EditorPartOptions = {
  openSideBySideDirection: 'right',
  splitOnDragAndDrop: true,
  alwaysShowEditorActions: false,
  closeEmptyGroups: true,
  splitSizing: 'auto',
};

export function readEditorPartOptions(): EditorPartOptions {
  const editor = useSettingsStore.getState().editor;
  return {
    openSideBySideDirection: editor.openSideBySideDirection ?? DEFAULT_EDITOR_PART_OPTIONS.openSideBySideDirection,
    splitOnDragAndDrop: editor.splitOnDragAndDrop ?? DEFAULT_EDITOR_PART_OPTIONS.splitOnDragAndDrop,
    alwaysShowEditorActions: editor.alwaysShowEditorActions ?? DEFAULT_EDITOR_PART_OPTIONS.alwaysShowEditorActions,
    closeEmptyGroups: editor.closeEmptyGroups ?? DEFAULT_EDITOR_PART_OPTIONS.closeEmptyGroups,
    splitSizing: editor.splitSizing ?? DEFAULT_EDITOR_PART_OPTIONS.splitSizing,
  };
}

export function preferSplitVerticallyFromDirection(direction: OpenSideBySideDirection): boolean {
  return direction === 'right';
}
