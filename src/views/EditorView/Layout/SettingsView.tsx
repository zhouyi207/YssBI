import React, { useState } from "react";
import { useSettingsStore } from "@/features/core/settings/settingsStore";
import { Select } from "@/shared/ui";

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
        if (window.confirm("确定要恢复所有默认设置吗？此操作不可撤销。")) {
            setIsResetting(true);
            try {
                await resetAllToDefaults();
            } finally {
                setIsResetting(false);
            }
        }
    };

    const handleResetSection = async (section: string) => {
        const sectionNames: Record<string, string> = {
            editor: "编辑器",
            appearance: "外观",
            color: "颜色/主题",
        };

        if (window.confirm(`确定要恢复${sectionNames[section] || section}的默认设置吗？`)) {
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
            } finally {
                setIsResetting(false);
            }
        }
    };

    if (isLoading) {
        return (
            <div className="w-full h-full bg-[#1e1e1e] text-[#cccccc] flex items-center justify-center">
                <div className="text-sm text-[#858585]">加载设置中...</div>
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
                                <h2 className="text-xl text-gray-100">Editor</h2>
                                <button
                                    onClick={() => handleResetSection("editor")}
                                    disabled={isResetting}
                                    className="px-3 py-1 text-xs bg-[#3c3c3c] hover:bg-[#4c4c4c] text-[#cccccc] rounded transition-colors disabled:opacity-50"
                                >
                                    恢复默认
                                </button>
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
                            <h2 className="text-xl text-gray-100 mb-6">Project</h2>
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
                                <h2 className="text-xl text-gray-100">Appearance</h2>
                                <button
                                    onClick={() => handleResetSection("appearance")}
                                    disabled={isResetting}
                                    className="px-3 py-1 text-xs bg-[#3c3c3c] hover:bg-[#4c4c4c] text-[#cccccc] rounded transition-colors disabled:opacity-50"
                                >
                                    恢复默认
                                </button>
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
                                <h2 className="text-xl text-gray-100">Colors</h2>
                                <button
                                    onClick={() => handleResetSection("color")}
                                    disabled={isResetting}
                                    className="px-3 py-1 text-xs bg-[#3c3c3c] hover:bg-[#4c4c4c] text-[#cccccc] rounded transition-colors disabled:opacity-50"
                                >
                                    恢复默认
                                </button>
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
        <div className="w-full h-full bg-[#1e1e1e] text-[#cccccc] flex flex-col overflow-hidden font-sans">
            {/* Header / Search Area */}
            <div className="h-12 border-b border-[#2b2b2b] flex items-center px-6 shrink-0 bg-[#1e1e1e]">
                <div className="flex-1 relative">
                    <input
                        type="text"
                        placeholder="Search settings"
                        className="w-full bg-[#3c3c3c] border border-transparent focus:border-[#007acc] outline-none rounded px-3 py-1 text-sm text-[#cccccc] placeholder-[#858585]"
                    />
                </div>
            </div>

            <div className="flex-1 flex overflow-hidden min-h-0">
                {/* Sidebar Navigation */}
                <aside className="w-64 border-r border-[#2b2b2b] bg-[#1e1e1e] pt-4 overflow-y-auto shrink-0">
                    <nav className="px-4 space-y-0.5">
                        {sections.map(section => (
                            <button
                                key={section.id}
                                onClick={() => setActiveSection(section.id)}
                                className={`
                                    w-full text-left px-3 py-1.5 rounded transition-colors text-sm
                                    ${activeSection === section.id
                                        ? 'bg-[#37373d] text-white font-semibold'
                                        : 'hover:bg-[#2a2d2e] text-[#cccccc]'}
                                `}
                            >
                                {section.label}
                            </button>
                        ))}
                    </nav>
                </aside>

                {/* Main Content Area */}
                <main className="flex-1 overflow-y-auto custom-scrollbar min-h-0">
                    <div className="max-w-4xl px-12 py-8">
                        {renderContent()}
                    </div>
                </main>
            </div>

            {/* 底部全局恢复默认设置按钮 */}
            <div className="h-12 border-t border-[#2b2b2b] flex items-center justify-end px-6 shrink-0 bg-[#1e1e1e]">
                <button
                    onClick={handleResetAll}
                    disabled={isResetting}
                    className="px-4 py-1.5 text-sm bg-[#c94f4f] hover:bg-[#d95f5f] text-white rounded transition-colors disabled:opacity-50"
                >
                    {isResetting ? "恢复中..." : "恢复所有默认设置"}
                </button>
            </div>

            <style dangerouslySetInnerHTML={{
                __html: `
                .custom-scrollbar::-webkit-scrollbar { width: 10px; }
                .custom-scrollbar::-webkit-scrollbar-track { background: transparent; }
                .custom-scrollbar::-webkit-scrollbar-thumb { background: #333333; }
                .custom-scrollbar::-webkit-scrollbar-thumb:hover { background: #444444; }
            `}} />
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
        <div className="group border-l-2 border-transparent hover:border-[#007acc] pl-4 transition-colors">
            <div className="mb-1 text-sm font-semibold text-[#cccccc] group-hover:text-[#007acc] transition-colors">{label}</div>
            <div className="text-xs text-[#858585] mb-3 leading-relaxed max-w-2xl">{description}</div>

            <div className="flex items-center">
                {type === "checkbox" && (
                    <input
                        type="checkbox"
                        checked={checked ?? true}
                        onChange={(e) => onChange?.(e.target.checked)}
                        className="w-4 h-4 rounded-sm border-[#3c3c3c] bg-[#3c3c3c] text-[#007acc] focus:ring-0 focus:ring-offset-0 cursor-pointer"
                    />
                )}
                {type === "text" && (
                    <input
                        type="text"
                        value={value ?? defaultValue ?? ""}
                        onChange={(e) => onChange?.(e.target.value)}
                        placeholder={placeholder}
                        disabled={disabled}
                        className="w-full max-w-md bg-[#3c3c3c] border border-transparent focus:border-[#007acc] outline-none rounded px-2 py-1 text-sm text-[#cccccc] disabled:opacity-50"
                    />
                )}
                {type === "number" && (
                    <input
                        type="number"
                        value={value ?? defaultValue ?? ""}
                        onChange={(e) => onChange?.(e.target.value)}
                        className="w-24 bg-[#3c3c3c] border border-transparent focus:border-[#007acc] outline-none rounded px-2 py-1 text-sm text-[#cccccc]"
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
                        <div className="relative w-10 h-6 rounded border border-[#3c3c3c] overflow-hidden">
                            <input
                                type="color"
                                value={value}
                                onChange={(e) => onChange?.(e.target.value)}
                                className="absolute -inset-1 w-12 h-8 p-0 border-none bg-transparent cursor-pointer"
                            />
                        </div>
                        <input
                            type="text"
                            value={value}
                            onChange={(e) => onChange?.(e.target.value)}
                            className="w-24 bg-[#3c3c3c] border border-transparent focus:border-[#007acc] outline-none rounded px-2 py-0.5 text-[11px] text-[#cccccc] font-mono"
                        />
                    </div>
                )}
            </div>
        </div>
    );
};
