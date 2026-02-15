import { useSettingsStore } from "@/features/core/settings/stores/settingsStore";

export const useEditorSettings = () =>
    useSettingsStore((s) => ({
        editor: s.editor,
        updateEditor: s.updateEditor,
    }));
