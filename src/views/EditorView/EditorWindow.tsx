import { useLayoutStore } from "@/features/core/layout/layoutStore";
import { useTranslation } from "react-i18next";
import { ActivityBar } from "./Layout/ActivityBar";
import { BottomBar } from "./Layout/BottomBar";
import { Menubar } from "./Layout/Menubar";
import { Workspace } from "./Layout/Workspace";
import { useAppInitialization } from "@/features/application/initialization";
import { LoadStatus } from "@/shared/types/ui";
import { useProjectSyncWithEditor } from "@/features/application/initialization";
import { useEditorGroup } from "@/features/application/editor";
import { useEditorKeyboard } from "@/features/application/editor";
import { useMenubar } from "@/features/application/menubar";
import { usePersistedWindow } from "@/features/application/window";
import { Dialog, DialogContent } from "@/components/ui/dialog";
import { SettingsView } from "./Layout/SettingsView";


export const EditorWindow = () => {
    const { t } = useTranslation();
    const rootId = useLayoutStore((s) => s.rootId);
    const isSettingsOpen = useLayoutStore((s) => s.isSettingsOpen);
    const setSettingsOpen = useLayoutStore((s) => s.setSettingsOpen);
    const { status, error } = useAppInitialization();

    // 主窗口几何状态：恢复尺寸/位置/最大化，并在关闭时持久化
    usePersistedWindow("main");

    // 启用项目同步（带编辑器回调，用于打开新 Tab 等 UI 扩展）
    useProjectSyncWithEditor();

    // 全局键盘快捷键（Ctrl+C/V/Z/Y 等），粘贴时使用鼠标位置
    const editor = useEditorGroup({ withCanvasInteraction: false });
    const { toggleLogPanel } = useMenubar();
    useEditorKeyboard({
        deleteSelected: editor.deleteSelected,
        undo: editor.undo,
        redo: editor.redo,
        copy: editor.copy,
        cut: editor.cut,
        paste: editor.paste,
        saveGraph: editor.saveGraph,
        importGraph: editor.importGraph,
        addEvent: editor.addEvent,
        closeTab: editor.closeTab,
        setActiveTabId: editor.setActiveTabId,
        splitEditorRight: editor.splitEditorRight,
        toggleLogPanel,
    });



    if (status !== LoadStatus.Ready) {
        return (
            <div className="flex items-center justify-center w-full h-screen">
                {error ? t("editor.initializationFailed", { error }) : t("common.loading")}
            </div>
        );
    }

    return (
        <div className="flex flex-col w-full h-screen">
            <Menubar />
            <div className="flex flex-1 overflow-hidden isolate">
                <ActivityBar />
                <Workspace nodeId={rootId} />
            </div>
            <BottomBar />
            <Dialog open={isSettingsOpen} onOpenChange={setSettingsOpen}>
                <DialogContent className="h-[min(760px,86vh)] max-w-[min(1120px,92vw)] p-0">
                    <SettingsView />
                </DialogContent>
            </Dialog>
        </div>
    );
}
