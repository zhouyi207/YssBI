import React, { useState } from "react";
import { useTheme } from "../Context/ThemeContext";
import { Select } from "../Shared/UI/Select";

export const SettingsView: React.FC = () => {
    const { theme, updateTheme } = useTheme();
    const [activeSection, setActiveSection] = useState("editor");

    const sections = [
        { id: "editor", label: "Editor" },
        { id: "project", label: "Project" },
        { id: "appearance", label: "Appearance" },
        { id: "color", label: "Color" }
    ];

    const renderContent = () => {
        switch (activeSection) {
            case "editor":
                return (
                    <div className="space-y-8">
                        <div>
                            <h2 className="text-xl text-gray-100 mb-6">Editor</h2>
                            <div className="space-y-6">
                                <SettingItem
                                    label="Show Grid"
                                    description="Controls whether the background grid is visible."
                                    type="checkbox"
                                />
                                <SettingItem
                                    label="Auto Save"
                                    description="Controls whether changed files are saved automatically after a delay."
                                    type="checkbox"
                                />
                                <SettingItem
                                    label="Snap to Grid"
                                    description="Controls whether nodes should snap to the grid corners when dragged."
                                    type="checkbox"
                                />
                                <SettingItem
                                    label="Font Size"
                                    description="Controls the font size in pixels for node titles and labels."
                                    type="number"
                                    defaultValue="12"
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
                                    defaultValue="YssBI Project"
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
                            <h2 className="text-xl text-gray-100 mb-6">Appearance</h2>
                            <div className="space-y-6">
                                <SettingItem
                                    label="Color Theme"
                                    description="Controls the overall color theme of the editor."
                                    type="select"
                                    options={["Dark Modern (Default)", "OLED Black", "Light Modern"]}
                                />
                                <SettingItem
                                    label="Activity Bar Position"
                                    description="Controls the visibility and position of the activity bar."
                                    type="select"
                                    options={["Left", "Right", "Hidden"]}
                                />
                                <SettingItem
                                    label="Smooth Scroll"
                                    description="Enable smooth scrolling in the canvas and menus."
                                    type="checkbox"
                                />
                            </div>
                        </div>
                    </div>
                );
            case "color":
                return (
                    <div className="space-y-8">
                        <div>
                            <h2 className="text-xl text-gray-100 mb-6">Colors</h2>

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
                                        label="Integer Color"
                                        description="Color for whole number data type pins."
                                        type="color"
                                        value={theme.intColor}
                                        onChange={(val: string) => updateTheme({ intColor: val })}
                                    />
                                    <SettingItem
                                        label="Float Color"
                                        description="Color for decimal number data type pins."
                                        type="color"
                                        value={theme.floatColor}
                                        onChange={(val: string) => updateTheme({ floatColor: val })}
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
    onChange?: (val: string) => void;
    placeholder?: string;
    disabled?: boolean;
    options?: string[];
}

const SettingItem: React.FC<SettingItemProps> = ({ label, description, type, defaultValue, value, onChange, placeholder, disabled, options }) => {
    return (
        <div className="group border-l-2 border-transparent hover:border-[#007acc] pl-4 transition-colors">
            <div className="mb-1 text-sm font-semibold text-[#cccccc] group-hover:text-[#007acc] transition-colors">{label}</div>
            <div className="text-xs text-[#858585] mb-3 leading-relaxed max-w-2xl">{description}</div>

            <div className="flex items-center">
                {type === "checkbox" && (
                    <input
                        type="checkbox"
                        defaultChecked
                        className="w-4 h-4 rounded-sm border-[#3c3c3c] bg-[#3c3c3c] text-[#007acc] focus:ring-0 focus:ring-offset-0 cursor-pointer"
                    />
                )}
                {type === "text" && (
                    <input
                        type="text"
                        defaultValue={defaultValue}
                        placeholder={placeholder}
                        disabled={disabled}
                        className="w-full max-w-md bg-[#3c3c3c] border border-transparent focus:border-[#007acc] outline-none rounded px-2 py-1 text-sm text-[#cccccc] disabled:opacity-50"
                    />
                )}
                {type === "number" && (
                    <input
                        type="number"
                        defaultValue={defaultValue}
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
