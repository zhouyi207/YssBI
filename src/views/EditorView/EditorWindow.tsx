import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { ActivityBar } from "./Layout/ActivityBar";
import { DragProvider } from "./Context/DragProvider";
import { DragLayer } from "./Layout/DragOverlay";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { UIHost } from "@/shared/ui/UIHost";
import { useEditorKeyboard } from "@/features/application/editor";
import { useEditor } from "@/features/application/editor";
import { useCallback, useMemo } from "react";
import { useViewportStore } from "@/features/core/viewport";
import { useLayoutStore as useLayoutStoreForKeyboard } from "@/features/core/layout/layoutStore";
import { useProjectSync } from "@/features/core/sync";
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';


export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { status, error } = useAppInitialization();
    const editor = useEditor();

    // 使用 useMemo 稳定回调引用，避免重复创建监听器
    const projectSyncCallbacks = useMemo(() => ({
        onEventCreated: editor.handleEventCreated,
        onEventCreatedFailed: editor.handleEventCreatedFailed,
        onFunctionCreated: editor.handleFunctionCreated,
        onFunctionCreatedFailed: editor.handleFunctionCreatedFailed,
        onMacroCreated: editor.handleMacroCreated,
        onMacroCreatedFailed: editor.handleMacroCreatedFailed,
        onNodeCreated: editor.handleNodeCreated,
        onNodeDeleted: editor.handleNodeDeleted,
    }), [
        editor.handleEventCreated,
        editor.handleEventCreatedFailed,
        editor.handleFunctionCreated,
        editor.handleFunctionCreatedFailed,
        editor.handleMacroCreated,
        editor.handleMacroCreatedFailed,
        editor.handleNodeCreated,
        editor.handleNodeDeleted,
    ]);

    // 启用项目同步（全局单例）并设置回调
    // 注意：这是应用中唯一调用 useProjectSync 的地方
    useProjectSync(projectSyncCallbacks);

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