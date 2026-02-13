import { useLayoutStore } from "@/features/layoutStore/layoutStore";
import { ActivityBar } from "./Layout/ActivityBar";
import { DragProvider } from "./Context/DragProvider";
import { DragLayer } from "./Layout/DragOverlay";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/editor/app-initialization";
import { LoadStatus } from "@/shared/types/loadStatus";
import { UIHost } from "@/shared/ui/uiHost";
import { useEditorKeyboard } from "@/features/editor/hooks/useEditorKeyboard";
import { useEditor } from "@/features/editor";
import { useCallback } from "react";
import { useViewportStore } from "@/features/canvas/stores";
import { useLayoutStore as useLayoutStoreForKeyboard } from "@/features/layoutStore/layoutStore";

const DEFAULT_VIEWPORT = { x: 0, y: 0, scale: 1 };

export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { status, error } = useAppInitialization();
    const editor = useEditor();

    // Helper to get active canvas local point for keyboard shortcuts
    const getActiveCanvasLocalPoint = useCallback((clientX: number, clientY: number) => {
        const gid = useLayoutStoreForKeyboard.getState().activeEditorGroupId || 
                    useLayoutStoreForKeyboard.getState().activeGroupId || 
                    'default_editor';
        const el = document.getElementById(`layout-node-${gid}`);
        if (!el) return { x: 0, y: 0 };
        const rect = el.getBoundingClientRect();
        const currentCanvas = useViewportStore.getState().viewports[gid] || DEFAULT_VIEWPORT;
        return {
            x: (clientX - rect.left - currentCanvas.x) / currentCanvas.scale,
            y: (clientY - rect.top - currentCanvas.y) / currentCanvas.scale
        };
    }, []);

    // Setup keyboard shortcuts
    useEditorKeyboard({
        deleteSelected: editor.deleteSelected,
        undo: editor.undo,
        redo: editor.redo,
        copy: editor.copy,
        cut: editor.cut,
        paste: editor.paste,
        saveGraph: editor.saveGraph,
        saveGraphAs: editor.saveGraphAs,
        importGraph: editor.importGraph,
        addEvent: editor.addEvent,
        closeTab: editor.closeTab,
        setActiveTabId: editor.setActiveTabId,
        splitEditorRight: editor.splitEditorRight,
        getActiveCanvasLocalPoint,
    });

    if (status !== LoadStatus.Ready) {
        return (
            <div className="flex items-center justify-center w-full h-screen">
                {error ? `初始化失败:${error}` : "加载中..."}
            </div>
        );
    }

    return (
        <>
            <DragProvider>
                <div className="flex flex-col w-full h-screen">
                    <Menubar />
                    <div className="flex flex-1 overflow-hidden">
                        <ActivityBar />
                        <Workspace nodeId={rootId} />
                    </div>
                    <DragLayer />
                </div>
            </DragProvider>
            <UIHost />
        </>
    );
}