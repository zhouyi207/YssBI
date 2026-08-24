export type OpenSideBySideDirection = 'right' | 'down';
export type EditorSplitSizingMode = 'auto' | 'distribute' | 'split';

export interface EditorSettings {
    showGrid: boolean;
    autoSave: boolean;
    snapToGrid: boolean;
    fontSize: number;
    /** VS Code `workbench.editor.openSideBySideDirection` */
    openSideBySideDirection?: OpenSideBySideDirection;
    /** VS Code `workbench.editor.splitOnDragAndDrop` */
    splitOnDragAndDrop?: boolean;
    /** VS Code `workbench.editor.alwaysShowEditorActions` */
    alwaysShowEditorActions?: boolean;
    /** VS Code `workbench.editor.closeEmptyGroups` */
    closeEmptyGroups?: boolean;
    /** VS Code `workbench.editor.splitSizing` */
    splitSizing?: EditorSplitSizingMode;

}