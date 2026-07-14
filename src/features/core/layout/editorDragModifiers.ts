const isMacintosh =
  typeof navigator !== 'undefined'
  && /Mac|iPod|iPhone|iPad/.test(navigator.platform);

/** VS Code `DropOverlay.isCopyOperation` */
export function isEditorDragCopyOperation(modifiers: { altKey: boolean; ctrlKey: boolean }): boolean {
  return isMacintosh ? modifiers.altKey : modifiers.ctrlKey;
}

/** VS Code `DropOverlay.isToggleSplitOperation` — inverts `splitOnDragAndDrop` for one drag. */
export function isEditorDragToggleSplitOperation(modifiers: { altKey: boolean; shiftKey: boolean }): boolean {
  return isMacintosh ? modifiers.shiftKey : modifiers.altKey;
}

export function resolveEnableSplittingOnDrag(
  splitOnDragAndDrop: boolean,
  modifiers: { altKey: boolean; shiftKey: boolean },
): boolean {
  const toggled = isEditorDragToggleSplitOperation(modifiers);
  return toggled ? !splitOnDragAndDrop : splitOnDragAndDrop;
}
