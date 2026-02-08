// editor/EditorApp.tsx

import { useLayoutStore } from "@/stores/layoutStore";

import ActivityBar from "./Layout/ActivityBar";
import { DragProvider } from "./Context/DragProvider";
import { DragLayer } from "./Layout/DragLayer";
import { CanvasProvider } from "./Context/CanvasProvider";
import Menubar from "./Layout/Menubar";
import { UIProvider } from "./Context/UIProvider";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "./Hooks/useAppInitialization";
import { useProjectSync } from "./Hooks/useProjectSync";

export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { isInitialized, error } = useAppInitialization();

    useProjectSync({ enabled: true });

    if (!isInitialized) {
        return (
            <div className="flex items-center justify-center w-full h-screen">
                {error ? "初始化失败" : "加载中..."}
            </div>
        );
    }

    return (
        <UIProvider>
            <DragProvider>
                <CanvasProvider>
                    <div className="flex flex-col w-full h-screen">
                        <Menubar />
                        <div className="flex flex-1 overflow-hidden">
                            <ActivityBar />
                            <Workspace nodeId={rootId} />
                        </div>
                        <DragLayer />
                    </div>
                </CanvasProvider>
            </DragProvider>
        </UIProvider>
    );
}