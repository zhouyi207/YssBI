import { useSettingsStore } from "@/stores/settingsStore";

export const useEditorSettings = () =>
    useSettingsStore((s) => ({
        editor: s.editor,
        updateEditor: s.updateEditor,
    }));
