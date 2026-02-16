import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { ActivityBar } from "./Layout/ActivityBar";
import { DragProvider } from "./Context/DragProvider";
import { DragLayer } from "./Layout/DragOverlay";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { UIHost } from "@/shared/ui";
import { useViewportStore } from "@/features/core/viewport";
import { useLayoutStore as useLayoutStoreForKeyboard } from "@/features/core/layout/layoutStore";
import { useProjectSync } from "@/features/core/sync";
import { DEFAULT_VIEWPORT } from '@/app/appConfig/default';
import { useCallback } from "react";


export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { status, error } = useAppInitialization();


    // 启用项目同步（全局单例）并设置回调
    // 注意：这是应用中唯一调用 useProjectSync 的地方
    useProjectSync();

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
