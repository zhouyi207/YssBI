import { useSettingsStore } from "@/features/core/settings/settingsStore";

export const useEditorSettings = () =>
    useSettingsStore((s) => ({
        editor: s.editor,
        updateEditor: s.updateEditor,
    }));
