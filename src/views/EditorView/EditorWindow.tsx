import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { ActivityBar } from "./Layout/ActivityBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { UIHost } from "@/shared/ui";
import { useViewportStore } from "@/features/core/viewport";
import { useLayoutStore as useLayoutStoreForKeyboard } from "@/features/core/layout/layoutStore";
import { useProjectSyncWithEditor } from "@/features/application/initialization";
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useCallback } from "react";


export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { status, error } = useAppInitialization();


    // 启用项目同步（带编辑器回调，用于打开新 Tab 等 UI 扩展）
    useProjectSyncWithEditor();

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



    if (status !== LoadStatus.Ready) {
        return (
            <div className="flex items-center justify-center w-full h-screen">
                {error ? `初始化失败:${error}` : "加载中..."}
            </div>
        );
    }

    return (
        <>
            <div className="flex flex-col w-full h-screen">
                <Menubar />
                <div className="flex flex-1 overflow-hidden">
                    <ActivityBar />
                    <Workspace nodeId={rootId} />
                </div>
            </div>
            <UIHost />
        </>
    );
}
