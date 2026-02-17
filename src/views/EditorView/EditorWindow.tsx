import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { ActivityBar } from "./Layout/ActivityBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { UIHost } from "@/shared/ui";
import { useProjectSyncWithEditor } from "@/features/application/initialization";
import { useEditorGroup } from "@/features/application/editor/core";
import { useEditorKeyboard } from "@/features/application/editor/core";


export const EditorWindow = () => {
    const rootId = useLayoutStore((s) => s.rootId);
    const { status, error } = useAppInitialization();

    // 启用项目同步（带编辑器回调，用于打开新 Tab 等 UI 扩展）
    useProjectSyncWithEditor();

    // 全局键盘快捷键（Ctrl+C/V/Z/Y 等），粘贴时使用鼠标位置
    const editor = useEditorGroup();
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
