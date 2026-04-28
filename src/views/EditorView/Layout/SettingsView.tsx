import React, { useState } from "react";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { uiStore } from "@/features/core/ui/UIStore";
import { Select } from "@/shared/ui";
import { OverlayScrollbar } from "@/shared/ui/OverlayScrollbar";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";

export const SettingsView: React.FC = () => {
    const theme = useSettingsStore((s) => s.theme);
    const editor = useSettingsStore((s) => s.editor);
    const appearance = useSettingsStore((s) => s.appearance);
    const project = useSettingsStore((s) => s.project);
    const isLoading = useSettingsStore((s) => s.isLoading);
    const updateTheme = useSettingsStore((s) => s.updateTheme);
    const updateEditor = useSettingsStore((s) => s.updateEditor);
    const updateAppearance = useSettingsStore((s) => s.updateAppearance);
    const updateProject = useSettingsStore((s) => s.updateProject);
    const resetAllToDefaults = useSettingsStore((s) => s.resetAllToDefaults);
    const resetThemeToDefaults = useSettingsStore((s) => s.resetThemeToDefaults);
    const resetEditorToDefaults = useSettingsStore((s) => s.resetEditorToDefaults);
    const resetAppearanceToDefaults = useSettingsStore((s) => s.resetAppearanceToDefaults);

    const [activeSection, setActiveSection] = useState("editor");
    const [isResetting, setIsResetting] = useState(false);

    const sections = [
        { id: "editor", label: "Editor" },
        { id: "project", label: "Project" },
        { id: "appearance", label: "Appearance" },
        { id: "color", label: "Color" }
    ];

    const handleResetAll = async () => {
        const confirmed = await uiStore.confirm({
            title: "恢复所有默认设置",
            message: "确定要恢复所有默认设置吗？此操作不可撤销。",
            type: "danger",
            confirmText: "恢复默认",
        });
        if (!confirmed) return;

        setIsResetting(true);
        try {
            await resetAllToDefaults();
            uiStore.showToast("已恢复所有默认设置", "success");
        } catch (error) {
            uiStore.showToast(`恢复默认设置失败: ${String(error)}`, "error");
        } finally {
            setIsResetting(false);
        }
    };

    const handleResetSection = async (section: string) => {
        const sectionNames: Record<string, string> = {
            editor: "编辑器",
            appearance: "外观",
            color: "颜色/主题",
        };

        const sectionName = sectionNames[section] || section;
        const confirmed = await uiStore.confirm({
            title: "恢复默认设置",
            message: `确定要恢复${sectionName}的默认设置吗？`,
            type: "danger",
            confirmText: "恢复默认",
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
            uiStore.showToast(`已恢复${sectionName}默认设置`, "success");
        } catch (error) {
            uiStore.showToast(`恢复${sectionName}默认设置失败: ${String(error)}`, "error");
        } finally {
            setIsResetting(false);
        }
    };

    if (isLoading) {
        return (
            <div className="w-full h-full bg-[var(--workbench-bg)] text-foreground flex items-center justify-center">
                <div className="text-sm text-muted-foreground">加载设置中...</div>
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
                                <h2 className="text-xl text-foreground">Editor</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("editor")}
                                    disabled={isResetting}
                                >
                                    恢复默认
                                </Button>
                            </div>
                            <div className="space-y-6">
                                <SettingItem
                                    label="Show Grid"
                                    description="Controls whether the background grid is visible."
                                    type="checkbox"
                                    checked={editor.showGrid}
                                    onChange={(val) => updateEditor({ showGrid: val === "true" || val === true })}
                                />
                                <SettingItem
                                    label="Auto Save"
                                    description="Controls whether changed files are saved automatically after a delay."
                                    type="checkbox"
                                    checked={editor.autoSave}
                                    onChange={(val) => updateEditor({ autoSave: val === "true" || val === true })}
                                />
                                <SettingItem
                                    label="Snap to Grid"
                                    description="Controls whether nodes should snap to the grid corners when dragged."
                                    type="checkbox"
                                    checked={editor.snapToGrid}
                                    onChange={(val) => updateEditor({ snapToGrid: val === "true" || val === true })}
                                />
                                <SettingItem
                                    label="Font Size"
                                    description="Controls the font size in pixels for node titles and labels."
                                    type="number"
                                    value={String(editor.fontSize)}
                                    onChange={(val) => updateEditor({ fontSize: parseInt(val as string) || 12 })}
                                />
                            </div>
                        </div>
                    </div>
                );
            case "project":
                return (
                    <div className="space-y-8">
                        <div>
                            <h2 className="text-xl text-foreground mb-6">Project</h2>
                            <div className="space-y-6">
                                <SettingItem
                                    label="Project Name"
                                    description="The name displayed in the title bar and used for exports."
                                    type="text"
                                    value={project.projectName}
                                    onChange={(val) => updateProject({ projectName: val as string })}
                                />
                                <SettingItem
                                    label="Project Version"
                                    description="The version of the project. This is managed by the system."
                                    type="text"
                                    defaultValue="1.0.0"
                                    disabled
                                />
                                <SettingItem
                                    label="Export Path"
                                    description="Default directory where the project will be exported."
                                    type="text"
                                    value={project.exportPath}
                                    onChange={(val) => updateProject({ exportPath: val as string })}
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
                                <h2 className="text-xl text-foreground">Appearance</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("appearance")}
                                    disabled={isResetting}
                                >
                                    恢复默认
                                </Button>
                            </div>
                            <div className="space-y-6">
                                <SettingItem
                                    label="Color Theme"
                                    description="Controls the overall color theme of the editor."
                                    type="select"
                                    options={["Dark Modern (Default)", "OLED Black", "Light Modern"]}
                                    value={appearance.colorTheme}
                                    onChange={(val) => updateAppearance({ colorTheme: val as string })}
                                />
                                <SettingItem
                                    label="Activity Bar Position"
                                    description="Controls the visibility and position of the activity bar."
                                    type="select"
                                    options={["Left", "Right", "Hidden"]}
                                    value={appearance.activityBarPosition}
                                    onChange={(val) => updateAppearance({ activityBarPosition: val as string })}
                                />
                                <SettingItem
                                    label="Smooth Scroll"
                                    description="Enable smooth scrolling in the canvas and menus."
                                    type="checkbox"
                                    checked={appearance.smoothScroll}
                                    onChange={(val) => updateAppearance({ smoothScroll: val === "true" || val === true })}
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
                                <h2 className="text-xl text-foreground">Colors</h2>
                                <Button
                                    type="button"
                                    variant="secondary"
                                    size="sm"
                                    onClick={() => handleResetSection("color")}
                                    disabled={isResetting}
                                >
                                    恢复默认
                                </Button>
                            </div>

                            <div className="mb-8">
                                <h3 className="text-[11px] font-bold text-[#858585] uppercase tracking-widest mb-4 opacity-70">Editor UI</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label="Workbench Background"
                                        description="The primary background color of the editor environment."
                                        type="color"
                                        value={theme.workbenchBackground}
                                        onChange={(val: string) => updateTheme({ workbenchBackground: val })}
                                    />
                                    <SettingItem
                                        label="Sidebar Background"
                                        description="Background color for sidebars and headers."
                                        type="color"
                                        value={theme.sidebarBackground}
                                        onChange={(val: string) => updateTheme({ sidebarBackground: val })}
                                    />
                                    <SettingItem
                                        label="Accent Color"
                                        description="The primary color used for selections and active highlights."
                                        type="color"
                                        value={theme.accentColor}
                                        onChange={(val: string) => updateTheme({ accentColor: val })}
                                    />
                                </div>
                            </div>

                            <div>
                                <h3 className="text-[11px] font-bold text-[#858585] uppercase tracking-widest mb-4 opacity-70">Canvas Elements</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label="Grid Lines"
                                        description="The color of the background grid in the graph editor."
                                        type="color"
                                        value={theme.gridLines}
                                        onChange={(val: string) => updateTheme({ gridLines: val })}
                                    />
                                    <SettingItem
                                        label="Node Base Color"
                                        description="The default background color for node bodies."
                                        type="color"
                                        value={theme.nodeBase}
                                        onChange={(val: string) => updateTheme({ nodeBase: val })}
                                    />
                                    <SettingItem
                                        label="Connection Lines"
                                        description="The base color for edges (links) between pins."
                                        type="color"
                                        value={theme.connectionLines}
                                        onChange={(val: string) => updateTheme({ connectionLines: val })}
                                    />
                                    <SettingItem
                                        label="Selection Region"
                                        description="The color of the drag-selection box."
                                        type="color"
                                        value={theme.selectionRegion}
                                        onChange={(val: string) => updateTheme({ selectionRegion: val })}
                                    />
                                </div>
                            </div>

                            <div>
                                <h3 className="text-[11px] font-bold text-[#858585] uppercase tracking-widest mb-4 opacity-70">Pin Colors</h3>
                                <div className="space-y-6">
                                    <SettingItem
                                        label="Execution Color"
                                        description="Color for execution flow pins and lines."
                                        type="color"
                                        value={theme.execColor}
                                        onChange={(val: string) => updateTheme({ execColor: val })}
                                    />
                                    <SettingItem
                                        label="Boolean Color"
                                        description="Color for boolean (true/false) data type pins."
                                        type="color"
                                        value={theme.boolColor}
                                        onChange={(val: string) => updateTheme({ boolColor: val })}
                                    />
                                    <SettingItem
                                        label="Int32 Color"
                                        description="Color for 32-bit integer pins."
                                        type="color"
                                        value={theme.int32Color}
                                        onChange={(val: string) => updateTheme({ int32Color: val })}
                                    />
                                    <SettingItem
                                        label="Int64 Color"
                                        description="Color for 64-bit integer pins."
                                        type="color"
                                        value={theme.int64Color}
                                        onChange={(val: string) => updateTheme({ int64Color: val })}
                                    />
                                    <SettingItem
                                        label="Float32 Color"
                                        description="Color for 32-bit float pins."
                                        type="color"
                                        value={theme.float32Color}
                                        onChange={(val: string) => updateTheme({ float32Color: val })}
                                    />
                                    <SettingItem
                                        label="Float64 Color"
                                        description="Color for 64-bit float pins."
                                        type="color"
                                        value={theme.float64Color}
                                        onChange={(val: string) => updateTheme({ float64Color: val })}
                                    />
                                    <SettingItem
                                        label="String Color"
                                        description="Color for text data type pins."
                                        type="color"
                                        value={theme.stringColor}
                                        onChange={(val: string) => updateTheme({ stringColor: val })}
                                    />
                                    <SettingItem
                                        label="Date Color"
                                        description="Color for date type pins."
                                        type="color"
                                        value={theme.dateColor}
                                        onChange={(val: string) => updateTheme({ dateColor: val })}
                                    />
                                    <SettingItem
                                        label="DateTime Color"
                                        description="Color for datetime type pins."
                                        type="color"
                                        value={theme.datetimeColor}
                                        onChange={(val: string) => updateTheme({ datetimeColor: val })}
                                    />
                                    <SettingItem
                                        label="Categorical Color"
                                        description="Color for categorical type pins."
                                        type="color"
                                        value={theme.categoricalColor}
                                        onChange={(val: string) => updateTheme({ categoricalColor: val })}
                                    />
                                    <SettingItem
                                        label="Object Color"
                                        description="Color for object and reference data type pins."
                                        type="color"
                                        value={theme.objectColor}
                                        onChange={(val: string) => updateTheme({ objectColor: val })}
                                    />
                                    <SettingItem
                                        label="Any Color"
                                        description="Color for untyped (Any) pins."
                                        type="color"
                                        value={theme.anyColor}
                                        onChange={(val: string) => updateTheme({ anyColor: val })}
                                    />
                                    <SettingItem
                                        label="OneOf Color"
                                        description="Color for union type (OneOf) pins, e.g. Float64 | String."
                                        type="color"
                                        value={theme.oneofColor}
                                        onChange={(val: string) => updateTheme({ oneofColor: val })}
                                    />
                                    <SettingItem
                                        label="DataFrame Color"
                                        description="Color for DataFrame pins."
                                        type="color"
                                        value={theme.dataframeColor}
                                        onChange={(val: string) => updateTheme({ dataframeColor: val })}
                                    />
                                    <SettingItem
                                        label="DataSeries Color"
                                        description="Color for DataSeries pins."
                                        type="color"
                                        value={theme.dataseriesColor}
                                        onChange={(val: string) => updateTheme({ dataseriesColor: val })}
                                    />
                                    <SettingItem
                                        label="Array Color"
                                        description="Color for Array pins."
                                        type="color"
                                        value={theme.arrayColor}
                                        onChange={(val: string) => updateTheme({ arrayColor: val })}
                                    />
                                    <SettingItem
                                        label="Struct Color"
                                        description="Color for user-defined Struct type pins."
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
                        placeholder="Search settings"
                        className="h-8"
                    />
                </div>
            </div>

            <div className="flex-1 flex overflow-hidden min-h-0">
                {/* Sidebar Navigation */}
                <aside className="w-64 border-r border-border bg-[var(--sidebar-bg)] shrink-0 flex flex-col min-h-0">
                    <OverlayScrollbar className="flex-1 pt-4 min-h-0" direction="vertical">
                    <nav className="px-4 space-y-0.5">
                        {sections.map(section => (
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
                    {isResetting ? "恢复中..." : "恢复所有默认设置"}
                </Button>
            </div>

        </div>
    );
};

interface SettingItemProps {
    label: string;
    description: string;
    type: "checkbox" | "text" | "number" | "select" | "color";
    defaultValue?: string;
    value?: string;
    checked?: boolean;
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    onChange?: (val: any) => void;
    placeholder?: string;
    disabled?: boolean;
    options?: string[];
}

const SettingItem: React.FC<SettingItemProps> = ({
    label,
    description,
    type,
    defaultValue,
    value,
    checked,
    onChange,
    placeholder,
    disabled,
    options
}) => {
    return (
        <div className="group border-l-2 border-transparent hover:border-[var(--accent-color)] pl-4 transition-colors">
            <div className="mb-1 text-sm font-semibold text-foreground group-hover:text-[var(--accent-color)] transition-colors">{label}</div>
            <div className="text-xs text-muted-foreground mb-3 leading-relaxed max-w-2xl">{description}</div>

            <div className="flex items-center">
                {type === "checkbox" && (
                    <Input
                        type="checkbox"
                        checked={checked ?? true}
                        onChange={(e) => onChange?.(e.target.checked)}
                        className="h-4 w-4 accent-[var(--accent-color)]"
                    />
                )}
                {type === "text" && (
                    <Input
                        type="text"
                        value={value ?? defaultValue ?? ""}
                        onChange={(e) => onChange?.(e.target.value)}
                        placeholder={placeholder}
                        disabled={disabled}
                        className="max-w-md"
                    />
                )}
                {type === "number" && (
                    <Input
                        type="number"
                        value={value ?? defaultValue ?? ""}
                        onChange={(e) => onChange?.(e.target.value)}
                        className="w-24"
                    />
                )}
                {type === "select" && (
                    <div className="w-full max-w-md">
                        <Select
                            options={options || []}
                            value={value || (options?.[0] || "")}
                            onChange={(val) => onChange?.(val)}
                        />
                    </div>
                )}
                {type === "color" && (
                    <div className="flex items-center gap-3">
                        <div className="relative w-10 h-6 rounded border border-border overflow-hidden">
                            <Input
                                type="color"
                                value={value}
                                onChange={(e) => onChange?.(e.target.value)}
                                className="absolute -inset-1 h-8 w-12 cursor-pointer border-none bg-transparent p-0"
                            />
                        </div>
                        <Input
                            type="text"
                            value={value}
                            onChange={(e) => onChange?.(e.target.value)}
                            className="h-7 w-24 font-mono text-[11px]"
                        />
                    </div>
                )}
            </div>
        </div>
    );
};
