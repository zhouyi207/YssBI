import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { Select } from "@/shared/ui";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { i18n, type AppLanguage } from "@/app/i18n";

export const SettingsView: React.FC = () => {
    const { t } = useTranslation();
    const theme = useSettingsStore((s) => s.theme);
    const editor = useSettingsStore((s) => s.editor);
    const appearance = useSettingsStore((s) => s.appearance);
    const project = useSettingsStore((s) => s.project);
    const isLoading = useSettingsStore((s) => s.isLoading);
    const updateTheme = useSettingsStore((s) => s.updateTheme);
    const updateEditor = useSettingsStore((s) => s.updateEditor);
    const updateAppearance = useSettingsStore((s) => s.updateAppearance);
    const updateProject = useSettingsStore((s) => s.updateProject);
    const saveDebounced = useSettingsStore((s) => s.saveDebounced);
    const resetAllToDefaults = useSettingsStore((s) => s.resetAllToDefaults);
    const resetThemeToDefaults = useSettingsStore((s) => s.resetThemeToDefaults);
    const resetEditorToDefaults = useSettingsStore((s) => s.resetEditorToDefaults);
    const resetAppearanceToDefaults = useSettingsStore((s) => s.resetAppearanceToDefaults);

    const [activeSection, setActiveSection] = useState("editor");
    const [isResetting, setIsResetting] = useState(false);
    const [searchQuery, setSearchQuery] = useState("");

    const sections = [
        { id: "editor", label: t("settings.sections.editor") },
        { id: "project", label: t("settings.sections.project") },
        { id: "appearance", label: t("settings.sections.appearance") },
        { id: "color", label: t("settings.sections.color") }
    ];

    const visibleSections = useMemo(() => {
        const query = searchQuery.trim().toLowerCase();
        if (!query) return sections;
        return sections.filter((section) => section.label.toLowerCase().includes(query));
    }, [sections, searchQuery]);

    useEffect(() => {
        if (visibleSections.length > 0 && !visibleSections.some((section) => section.id === activeSection)) {
            setActiveSection(visibleSections[0].id);
        }
    }, [activeSection, visibleSections]);

    const languageOptions = [
        { label: t("language.zhCN"), value: "zh-CN" },
        { label: t("language.enUS"), value: "en-US" },
    ];

    const themeOptions = [
        { label: t("settings.options.darkModern"), value: "Dark Modern (Default)" },
        { label: t("settings.options.oledBlack"), value: "OLED Black" },
        { label: t("settings.options.lightModern"), value: "Light Modern" },
    ];

    const activityBarOptions = [
        { label: t("settings.options.left"), value: "Left" },
        { label: t("settings.options.right"), value: "Right" },
        { label: t("settings.options.hidden"), value: "Hidden" },
    ];

    const panelPositionOptions = [
        { label: t("settings.options.bottom"), value: "Bottom" },
        { label: t("settings.options.left"), value: "Left" },
        { label: t("settings.options.right"), value: "Right" },
    ];

    const titleBarStyleOptions = [
        { label: t("settings.options.titleBarCustom"), value: "custom" },
        { label: t("settings.options.titleBarNative"), value: "native" },
    ];

    const handleResetAll = async () => {
        const confirmed = await uiStore.confirm({
            title: t("settings.confirmResetAllTitle"),
            message: t("settings.confirmResetAllMessage"),
            type: "danger",
            confirmText: t("common.restoreDefaults"),
        });
        if (!confirmed) return;

        setIsResetting(true);
        try {
            await resetAllToDefaults();
            uiStore.showToast(t("settings.restoredAll"), "success");
        } catch (error) {
            uiStore.showToast(t("settings.restoreAllFailed", { error: String(error) }), "error");
        } finally {
            setIsResetting(false);
        }
    };

    const handleResetSection = async (section: string) => {
        const sectionNames: Record<string, string> = {
            editor: t("settings.sections.editor"),
            appearance: t("settings.sections.appearance"),
            color: t("settings.sections.color"),
        };

        const sectionName = sectionNames[section] || section;
        const confirmed = await uiStore.confirm({
            title: t("settings.confirmResetTitle"),
            message: t("settings.confirmResetMessage", { section: sectionName }),
            type: "danger",
            confirmText: t("common.restoreDefaults"),
        });
        if (!confirmed) return;

        setIsResetting(true);
        try {
            switch (section) {
                case "editor":
                    await resetEditorToDefaults();
                    break;
                case "appearance":
                    await resetAppearanceToDefaults();
                    break;
                case "color":
                    await resetThemeToDefaults();
                    break;
            }
            uiStore.showToast(t("settings.restoredSection", { section: sectionName }), "success");
        } catch (error) {
            uiStore.showToast(t("settings.restoreSectionFailed", { section: sectionName, error: String(error) }), "error");
        } finally {
            setIsResetting(false);
        }
    };

    if (isLoading) {
        return (
            <div className="w-full h-full bg-[var(--workbench-bg)] text-foreground flex items-center justify-center">
                <div className="text-sm text-muted-foreground">{t("settings.loading")}</div>
            </div>
        );
    }

    const renderContent = () => {
        switch (activeSection) {
            case "editor":
                return (
                    <div className="space-y-8">
                        <div>
                            <div className="flex items-center justify-between mb-6">
                                <h2 className="text-xl text-foreground">{t("settings.sections.editor")}</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("editor")}
                                    disabled={isResetting}
                                >
                                    {t("common.restoreDefaults")}
                                </Button>
                            </div>
                            <div className="space-y-6">
                                <SettingItem
                                    label={t("settings.labels.showGrid")}
                                    description={t("settings.descriptions.showGrid")}
                                    type="checkbox"
                                    checked={editor.showGrid}
                                    onChange={(val) => updateEditor({ showGrid: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.autoSave")}
                                    description={t("settings.descriptions.autoSave")}
                                    type="checkbox"
                                    checked={editor.autoSave}
                                    onChange={(val) => updateEditor({ autoSave: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.snapToGrid")}
                                    description={t("settings.descriptions.snapToGrid")}
                                    type="checkbox"
                                    checked={editor.snapToGrid}
                                    onChange={(val) => updateEditor({ snapToGrid: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.fontSize")}
                                    description={t("settings.descriptions.fontSize")}
                                    type="number"
                                    value={String(editor.fontSize)}
                                    onChange={(val) => updateEditor({ fontSize: parseInt(val, 10) || 12 })}
                                />
                            </div>
                        </div>
                    </div>
                );
            case "project":
                return (
                    <div className="space-y-8">
                        <div>
                            <h2 className="text-xl text-foreground mb-6">{t("settings.sections.project")}</h2>
                            <div className="space-y-6">
                                <SettingItem
                                    label={t("settings.labels.projectName")}
                                    description={t("settings.descriptions.projectName")}
                                    type="text"
                                    value={project.projectName}
                                    onChange={(val) => updateProject({ projectName: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.projectVersion")}
                                    description={t("settings.descriptions.projectVersion")}
                                    type="text"
                                    defaultValue="1.0.0"
                                    disabled
                                />
                                <SettingItem
                                    label={t("settings.labels.exportPath")}
                                    description={t("settings.descriptions.exportPath")}
                                    type="text"
                                    value={project.exportPath}
                                    onChange={(val) => updateProject({ exportPath: val })}
                                    placeholder="/path/to/export"
                                />
                            </div>
                        </div>
                    </div>
                );
            case "appearance":
                return (
                    <div className="space-y-8">
                        <div>
                            <div className="flex items-center justify-between mb-6">
                                <h2 className="text-xl text-foreground">{t("settings.sections.appearance")}</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("appearance")}
                                    disabled={isResetting}
                                >
                                    {t("common.restoreDefaults")}
                                </Button>
                            </div>
                            <div className="space-y-6">
                                <SettingItem
                                    label={t("settings.labels.colorTheme")}
                                    description={t("settings.descriptions.colorTheme")}
                                    type="select"
                                    options={themeOptions}
                                    value={appearance.colorTheme}
                                    onChange={(val) => updateAppearance({ colorTheme: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.language")}
                                    description={t("settings.descriptions.language")}
                                    type="select"
                                    options={languageOptions}
                                    value={appearance.language}
                                    onChange={(val) => {
                                        const language = val as AppLanguage;
                                        updateAppearance({ language });
                                        void i18n.changeLanguage(language);
                                        saveDebounced();
                                    }}
                                />
                                <SettingItem
                                    label={t("settings.labels.activityBarPosition")}
                                    description={t("settings.descriptions.activityBarPosition")}
                                    type="select"
                                    options={activityBarOptions}
                                    value={appearance.activityBarPosition}
                                    onChange={(val) => updateAppearance({ activityBarPosition: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.panelPosition")}
                                    description={t("settings.descriptions.panelPosition")}
                                    type="select"
                                    options={panelPositionOptions}
                                    value={appearance.panelPosition}
                                    onChange={(val) => {
                                        updateAppearance({ panelPosition: val });
                                        saveDebounced();
                                    }}
                                />
                                <SettingItem
                                    label={t("settings.labels.titleBarStyle")}
                                    description={t("settings.descriptions.titleBarStyle")}
                                    type="select"
                                    options={titleBarStyleOptions}
                                    value={appearance.titleBarStyle}
                                    onChange={(val) => {
                                        updateAppearance({ titleBarStyle: val as "custom" | "native" });
                                        saveDebounced();
                                    }}
                                />
                                <SettingItem
                                    label={t("settings.labels.smoothScroll")}
                                    description={t("settings.descriptions.smoothScroll")}
                                    type="checkbox"
                                    checked={appearance.smoothScroll}
                                    onChange={(val) => updateAppearance({ smoothScroll: val })}
                                />
                            </div>
                        </div>
                    </div>
                );
            case "color":
                return (
                    <div className="space-y-8">
                        <div>
                            <div className="flex items-center justify-between mb-6">
                                <h2 className="text-xl text-foreground">{t("settings.sections.color")}</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("color")}
                                    disabled={isResetting}
                                >
                                    {t("common.restoreDefaults")}
                                </Button>
                            </div>

                            <div className="mb-8">
                                <h3 className="mb-4 text-[11px] font-bold uppercase tracking-widest text-muted-foreground opacity-70">{t("settings.groups.editorUi")}</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label={t("settings.labels.workbenchBackground")}
                                        description={t("settings.descriptions.workbenchBackground")}
                                        type="color"
                                        value={theme.workbenchBackground}
                                        onChange={(val: string) => updateTheme({ workbenchBackground: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.sidebarBackground")}
                                        description={t("settings.descriptions.sidebarBackground")}
                                        type="color"
                                        value={theme.sidebarBackground}
                                        onChange={(val: string) => updateTheme({ sidebarBackground: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.accentColor")}
                                        description={t("settings.descriptions.accentColor")}
                                        type="color"
                                        value={theme.accentColor}
                                        onChange={(val: string) => updateTheme({ accentColor: val })}
                                    />
                                </div>
                            </div>

                            <div>
                                <h3 className="mb-4 text-[11px] font-bold uppercase tracking-widest text-muted-foreground opacity-70">{t("settings.groups.canvasElements")}</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label={t("settings.labels.gridLines")}
                                        description={t("settings.descriptions.gridLines")}
                                        type="color"
                                        value={theme.gridLines}
                                        onChange={(val: string) => updateTheme({ gridLines: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.nodeBaseColor")}
                                        description={t("settings.descriptions.nodeBaseColor")}
                                        type="color"
                                        value={theme.nodeBase}
                                        onChange={(val: string) => updateTheme({ nodeBase: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.connectionLines")}
                                        description={t("settings.descriptions.connectionLines")}
                                        type="color"
                                        value={theme.connectionLines}
                                        onChange={(val: string) => updateTheme({ connectionLines: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.selectionRegion")}
                                        description={t("settings.descriptions.selectionRegion")}
                                        type="color"
                                        value={theme.selectionRegion}
                                        onChange={(val: string) => updateTheme({ selectionRegion: val })}
                                    />
                                </div>
                            </div>

                            <div>
                                <h3 className="mb-4 text-[11px] font-bold uppercase tracking-widest text-muted-foreground opacity-70">{t("settings.groups.pinColors")}</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label={t("settings.labels.executionColor")}
                                        description={t("settings.descriptions.executionColor")}
                                        type="color"
                                        value={theme.execColor}
                                        onChange={(val: string) => updateTheme({ execColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.booleanColor")}
                                        description={t("settings.descriptions.booleanColor")}
                                        type="color"
                                        value={theme.boolColor}
                                        onChange={(val: string) => updateTheme({ boolColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.int32Color")}
                                        description={t("settings.descriptions.int32Color")}
                                        type="color"
                                        value={theme.int32Color}
                                        onChange={(val: string) => updateTheme({ int32Color: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.int64Color")}
                                        description={t("settings.descriptions.int64Color")}
                                        type="color"
                                        value={theme.int64Color}
                                        onChange={(val: string) => updateTheme({ int64Color: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.float32Color")}
                                        description={t("settings.descriptions.float32Color")}
                                        type="color"
                                        value={theme.float32Color}
                                        onChange={(val: string) => updateTheme({ float32Color: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.float64Color")}
                                        description={t("settings.descriptions.float64Color")}
                                        type="color"
                                        value={theme.float64Color}
                                        onChange={(val: string) => updateTheme({ float64Color: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.stringColor")}
                                        description={t("settings.descriptions.stringColor")}
                                        type="color"
                                        value={theme.stringColor}
                                        onChange={(val: string) => updateTheme({ stringColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.dateColor")}
                                        description={t("settings.descriptions.dateColor")}
                                        type="color"
                                        value={theme.dateColor}
                                        onChange={(val: string) => updateTheme({ dateColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.dateTimeColor")}
                                        description={t("settings.descriptions.dateTimeColor")}
                                        type="color"
                                        value={theme.datetimeColor}
                                        onChange={(val: string) => updateTheme({ datetimeColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.categoricalColor")}
                                        description={t("settings.descriptions.categoricalColor")}
                                        type="color"
                                        value={theme.categoricalColor}
                                        onChange={(val: string) => updateTheme({ categoricalColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.objectColor")}
                                        description={t("settings.descriptions.objectColor")}
                                        type="color"
                                        value={theme.objectColor}
                                        onChange={(val: string) => updateTheme({ objectColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.anyColor")}
                                        description={t("settings.descriptions.anyColor")}
                                        type="color"
                                        value={theme.anyColor}
                                        onChange={(val: string) => updateTheme({ anyColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.oneOfColor")}
                                        description={t("settings.descriptions.oneOfColor")}
                                        type="color"
                                        value={theme.oneofColor}
                                        onChange={(val: string) => updateTheme({ oneofColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.dataFrameColor")}
                                        description={t("settings.descriptions.dataFrameColor")}
                                        type="color"
                                        value={theme.dataframeColor}
                                        onChange={(val: string) => updateTheme({ dataframeColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.dataSeriesColor")}
                                        description={t("settings.descriptions.dataSeriesColor")}
                                        type="color"
                                        value={theme.dataseriesColor}
                                        onChange={(val: string) => updateTheme({ dataseriesColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.arrayColor")}
                                        description={t("settings.descriptions.arrayColor")}
                                        type="color"
                                        value={theme.arrayColor}
                                        onChange={(val: string) => updateTheme({ arrayColor: val })}
                                    />
                                    <SettingItem
                                        label={t("settings.labels.structColor")}
                                        description={t("settings.descriptions.structColor")}
                                        type="color"
                                        value={theme.structColor}
                                        onChange={(val: string) => updateTheme({ structColor: val })}
                                    />
                                </div>
                            </div>
                        </div>
                    </div>
                );
            default:
                return null;
        }
    };

    return (
        <div className="w-full h-full bg-[var(--workbench-bg)] text-foreground flex flex-col overflow-hidden font-sans">
            {/* Header / Search Area */}
            <div className="h-12 border-b border-border flex items-center px-6 shrink-0 bg-[var(--workbench-bg)]">
                <div className="flex-1 relative">
                    <Input
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        placeholder={t("settings.searchPlaceholder")}
                        className="h-8"
                    />
                </div>
            </div>

            <div className="flex-1 flex overflow-hidden min-h-0">
                {/* Sidebar Navigation */}
                <aside className="w-64 border-r border-border bg-[var(--sidebar-bg)] shrink-0 flex flex-col min-h-0">
                    <OverlayScrollbar className="flex-1 pt-4 min-h-0" direction="vertical">
                    <nav className="px-4 space-y-0.5">
                        {visibleSections.map(section => (
                            <Button
                                type="button"
                                variant={activeSection === section.id ? "secondary" : "ghost"}
                                key={section.id}
                                onClick={() => setActiveSection(section.id)}
                                className="w-full justify-start"
                            >
                                {section.label}
                            </Button>
                        ))}
                    </nav>
                    </OverlayScrollbar>
                </aside>

                {/* Main Content Area */}
                <main className="flex-1 min-h-0 flex flex-col">
                    <OverlayScrollbar className="flex-1 min-h-0" direction="vertical">
                    <div className="max-w-4xl px-12 py-8">
                        {renderContent()}
                    </div>
                    </OverlayScrollbar>
                </main>
            </div>

            {/* 底部全局恢复默认设置按钮 */}
            <div className="h-12 border-t border-border flex items-center justify-end px-6 shrink-0 bg-[var(--workbench-bg)]">
                <Button
                    type="button"
                    variant="destructive"
                    onClick={handleResetAll}
                    disabled={isResetting}
                >
                    {isResetting ? t("common.restoring") : t("common.restoreAllDefaults")}
                </Button>
            </div>

        </div>
    );
};

interface SettingItemBase {
    label: string;
    description: string;
    placeholder?: string;
    disabled?: boolean;
}

type SettingItemProps =
    | (SettingItemBase & {
        type: "checkbox";
        checked?: boolean;
        onChange?: (val: boolean) => void;
    })
    | (SettingItemBase & {
        type: "text";
        value?: string;
        defaultValue?: string;
        onChange?: (val: string) => void;
    })
    | (SettingItemBase & {
        type: "number";
        value?: string;
        defaultValue?: string;
        onChange?: (val: string) => void;
    })
    | (SettingItemBase & {
        type: "select";
        value?: string;
        options?: Array<{ label: string; value: string }>;
        onChange?: (val: string) => void;
    })
    | (SettingItemBase & {
        type: "color";
        value?: string;
        onChange?: (val: string) => void;
    });

const SettingItem: React.FC<SettingItemProps> = (props) => {
    const { label, description, type, placeholder, disabled } = props;
    const controlId = React.useId();

    return (
        <div className="group border-l-2 border-transparent hover:border-[var(--accent-color)] pl-4 transition-colors">
            <label htmlFor={controlId} className="mb-1 block text-sm font-semibold text-foreground group-hover:text-[var(--accent-color)] transition-colors">{label}</label>
            <div className="text-xs text-muted-foreground mb-3 leading-relaxed max-w-2xl">{description}</div>

            <div className="flex items-center">
                {type === "checkbox" && (
                    <Checkbox
                        id={controlId}
                        checked={props.checked ?? false}
                        onCheckedChange={(value) => props.onChange?.(value === true)}
                    />
                )}
                {type === "text" && (
                    <Input
                        id={controlId}
                        type="text"
                        value={props.value ?? props.defaultValue ?? ""}
                        onChange={(e) => props.onChange?.(e.target.value)}
                        placeholder={placeholder}
                        disabled={disabled}
                        className="max-w-md"
                    />
                )}
                {type === "number" && (
                    <Input
                        id={controlId}
                        type="number"
                        value={props.value ?? props.defaultValue ?? ""}
                        onChange={(e) => props.onChange?.(e.target.value)}
                        className="w-24"
                    />
                )}
                {type === "select" && (
                    <div className="w-full max-w-md">
                        <Select
                            id={controlId}
                            options={props.options || []}
                            value={props.value || (props.options?.[0]?.value || "")}
                            onChange={(val) => props.onChange?.(val)}
                        />
                    </div>
                )}
                {type === "color" && (
                    <div className="flex items-center gap-3">
                        <div className="relative w-10 h-6 rounded border border-border overflow-hidden">
                            <Input
                                id={controlId}
                                type="color"
                                value={props.value}
                                onChange={(e) => props.onChange?.(e.target.value)}
                                className="absolute -inset-1 h-8 w-12 cursor-pointer border-none bg-transparent p-0"
                            />
                        </div>
                        <Input
                            aria-label={label}
                            type="text"
                            value={props.value}
                            onChange={(e) => props.onChange?.(e.target.value)}
                            className="h-7 w-24 font-mono text-[11px]"
                        />
                    </div>
                )}
            </div>
        </div>
    );
};
