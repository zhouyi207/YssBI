import React, { useEffect, useMemo, useState } from "react";
import { VscError } from 'react-icons/vsc';
import { useTranslation } from "react-i18next";
import { useSettingsStore, uiStore } from "@/features/application/viewCapabilities";
import { Select } from "@/shared/ui";
import { Alert, AlertDescription } from '@/components/ui/alert';
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { i18n, type AppLanguage } from "@/app/i18n";
import { useProjectComputationSettings } from '@/features/application/projectSettings/useProjectComputationSettings';
import { formatInlineUserError } from '@/features/application/userErrorSummary';

interface SettingsViewProps {
    onRequestClose?: () => void;
    onDirtyChange?: (dirty: boolean) => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({ onRequestClose, onDirtyChange }) => {
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
    const resetAllToDefaults = useSettingsStore((s) => s.resetAllToDefaults);
    const resetThemeToDefaults = useSettingsStore((s) => s.resetThemeToDefaults);
    const resetEditorToDefaults = useSettingsStore((s) => s.resetEditorToDefaults);
    const resetAppearanceToDefaults = useSettingsStore((s) => s.resetAppearanceToDefaults);

    const computation = useProjectComputationSettings();
    const [activeSection, setActiveSection] = useState("editor");
    const [isResetting, setIsResetting] = useState(false);
    const [resetAllError, setResetAllError] = useState<string | null>(null);
    const [sectionResetError, setSectionResetError] = useState<{ section: string; message: string } | null>(null);
    const [searchQuery, setSearchQuery] = useState("");

    const sections = [
        { id: "editor", label: t("settings.sections.editor") },
        { id: "project", label: t("settings.sections.project") },
        { id: "computation", label: t("settings.sections.computation") },
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

    useEffect(() => {
        onDirtyChange?.(computation.isDirty);
        return () => onDirtyChange?.(false);
    }, [computation.isDirty, onDirtyChange]);

    const confirmDiscardComputation = async (): Promise<boolean> => {
        if (!computation.isDirty) return true;
        return uiStore.confirm({
            title: "Discard computation changes?",
            message: "Your unapplied project computation settings will be lost.",
            confirmText: "Discard",
            cancelText: "Keep Editing",
            type: "danger",
        });
    };

    const requestSection = async (section: string) => {
        if (section === activeSection) return;
        if (!(await confirmDiscardComputation())) return;
        setActiveSection(section);
    };

    const requestClose = async () => {
        if (!(await confirmDiscardComputation())) return;
        onRequestClose?.();
    };

    const languageOptions = [
        { label: t("language.zhCN"), value: "zh-CN" },
        { label: t("language.enUS"), value: "en-US" },
    ];

    const themeOptions = [
        { label: t("settings.options.darkModern"), value: "Dark Modern (Default)" },
        { label: t("settings.options.oledBlack"), value: "OLED Black" },
        { label: t("settings.options.lightModern"), value: "Light Modern" },
    ];


    const titleBarStyleOptions = [
        { label: t("settings.options.titleBarCustom"), value: "custom" },
        { label: t("settings.options.titleBarNative"), value: "native" },
    ];

    const openSideBySideDirectionOptions = [
        { label: t("settings.options.openSideBySideRight"), value: "right" },
        { label: t("settings.options.openSideBySideDown"), value: "down" },
    ];

    const splitSizingOptions = [
        { label: t("settings.options.splitSizingAuto"), value: "auto" },
        { label: t("settings.options.splitSizingDistribute"), value: "distribute" },
        { label: t("settings.options.splitSizingSplit"), value: "split" },
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
        setResetAllError(null);
        setSectionResetError(null);
        try {
            await resetAllToDefaults();
        } catch (error) {
            setResetAllError(t("settings.restoreAllFailed", {
                error: formatInlineUserError(error, t),
            }));
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
        setSectionResetError((current) => current?.section === section ? null : current);
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
        } catch (error) {
            setSectionResetError({
                section,
                message: t("settings.restoreSectionFailed", {
                    section: sectionName,
                    error: formatInlineUserError(error, t),
                }),
            });
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
                        <div>
                            <h3 className="text-base font-semibold text-foreground mb-4">
                                {t("settings.sections.editorGroups")}
                            </h3>
                            <div className="space-y-6">
                                <SettingItem
                                    label={t("settings.labels.openSideBySideDirection")}
                                    description={t("settings.descriptions.openSideBySideDirection")}
                                    type="select"
                                    options={openSideBySideDirectionOptions}
                                    value={editor.openSideBySideDirection ?? "right"}
                                    onChange={(val) => updateEditor({ openSideBySideDirection: val as "right" | "down" })}
                                />
                                <SettingItem
                                    label={t("settings.labels.splitOnDragAndDrop")}
                                    description={t("settings.descriptions.splitOnDragAndDrop")}
                                    type="checkbox"
                                    checked={editor.splitOnDragAndDrop ?? true}
                                    onChange={(val) => updateEditor({ splitOnDragAndDrop: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.alwaysShowEditorActions")}
                                    description={t("settings.descriptions.alwaysShowEditorActions")}
                                    type="checkbox"
                                    checked={editor.alwaysShowEditorActions ?? false}
                                    onChange={(val) => updateEditor({ alwaysShowEditorActions: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.closeEmptyGroups")}
                                    description={t("settings.descriptions.closeEmptyGroups")}
                                    type="checkbox"
                                    checked={editor.closeEmptyGroups ?? true}
                                    onChange={(val) => updateEditor({ closeEmptyGroups: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.splitSizing")}
                                    description={t("settings.descriptions.splitSizing")}
                                    type="select"
                                    options={splitSizingOptions}
                                    value={editor.splitSizing ?? "auto"}
                                    onChange={(val) => updateEditor({ splitSizing: val as "auto" | "distribute" | "split" })}
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
            case "computation":
                return (
                    <fieldset
                        role="group"
                        aria-label="settings.computation.groupLabel"
                        aria-disabled={!computation.enabled}
                        disabled={!computation.enabled || computation.isLoading || computation.isApplying}
                        className="space-y-6"
                    >
                        <div>
                            <h2 className="mb-2 text-xl text-foreground">{t("settings.sections.computation")}</h2>
                            <p className="text-sm text-muted-foreground">
                                Project-authoritative numeric comparison and statistical missing-value behavior.
                            </p>
                        </div>
                        <SettingItem
                            label="Absolute tolerance"
                            description="The fixed lower bound used for approximate numeric equality."
                            type="text"
                            value={computation.draft.absolute}
                            onChange={(absolute) => computation.setDraft({ absolute })}
                            disabled={!computation.enabled}
                        />
                        <SettingItem
                            label="Relative tolerance"
                            description="The scale-dependent bound used for approximate numeric equality."
                            type="text"
                            value={computation.draft.relative}
                            onChange={(relative) => computation.setDraft({ relative })}
                            disabled={!computation.enabled}
                        />
                        <div className="rounded-md border border-border bg-muted/20 p-3 font-mono text-xs text-muted-foreground">
                            |a - b| ≤ max(absolute, relative × max(|a|, |b|))
                        </div>
                        <SettingItem
                            label="Statistical missing values"
                            description="Listwise removes rows containing missing values; Reject reports an error."
                            type="select"
                            value={computation.draft.statistics}
                            options={[
                                { label: "Listwise", value: "listwise" },
                                { label: "Reject", value: "reject" },
                            ]}
                            onChange={(statistics) => computation.setDraft({
                                statistics: statistics as "listwise" | "reject",
                            })}
                            disabled={!computation.enabled}
                        />
                        {computation.validationError && (
                            <p role="alert" className="text-sm text-destructive">{computation.validationError}</p>
                        )}
                        {computation.error && (
                            <p role="alert" className="text-sm text-destructive">{computation.error}</p>
                        )}
                        <div className="flex items-center justify-end gap-2">
                            <Button
                                type="button"
                                variant="secondary"
                                onClick={computation.restoreRecommended}
                                disabled={!computation.enabled || computation.isApplying}
                            >
                                Restore Recommended Values
                            </Button>
                            <Button
                                type="button"
                                onClick={() => void computation.apply()}
                                disabled={!computation.enabled || !computation.isDirty
                                    || Boolean(computation.validationError) || computation.isApplying}
                            >
                                {computation.isApplying ? "Applying…" : "Apply"}
                            </Button>
                        </div>
                    </fieldset>
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

                            <ColorGroup title={t("settings.groups.surfaces")}>
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
                                    label={t("settings.labels.nodeBackground")}
                                    description={t("settings.descriptions.nodeBackground")}
                                    type="color"
                                    value={theme.nodeBackground}
                                    onChange={(val: string) => updateTheme({ nodeBackground: val })}
                                />
                            </ColorGroup>

                            <ColorGroup title={t("settings.groups.content")}>
                                <SettingItem
                                    label={t("settings.labels.foreground")}
                                    description={t("settings.descriptions.foreground")}
                                    type="color"
                                    value={theme.foreground}
                                    onChange={(val: string) => updateTheme({ foreground: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.mutedForeground")}
                                    description={t("settings.descriptions.mutedForeground")}
                                    type="color"
                                    value={theme.mutedForeground}
                                    onChange={(val: string) => updateTheme({ mutedForeground: val })}
                                />
                            </ColorGroup>

                            <ColorGroup title={t("settings.groups.interaction")}>
                                <SettingItem
                                    label={t("settings.labels.accentColor")}
                                    description={t("settings.descriptions.accentColor")}
                                    type="color"
                                    value={theme.accentColor}
                                    onChange={(val: string) => updateTheme({ accentColor: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.selectionColor")}
                                    description={t("settings.descriptions.selectionColor")}
                                    type="color"
                                    value={theme.selectionColor}
                                    onChange={(val: string) => updateTheme({ selectionColor: val })}
                                />
                            </ColorGroup>

                            <ColorGroup title={t("settings.groups.structure")}>
                                <SettingItem
                                    label={t("settings.labels.borderColor")}
                                    description={t("settings.descriptions.borderColor")}
                                    type="color"
                                    value={theme.borderColor}
                                    onChange={(val: string) => updateTheme({ borderColor: val })}
                                />
                                <SettingItem
                                    label={t("settings.labels.gridColor")}
                                    description={t("settings.descriptions.gridColor")}
                                    type="color"
                                    value={theme.gridColor}
                                    onChange={(val: string) => updateTheme({ gridColor: val })}
                                />
                            </ColorGroup>
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
            <div className="h-12 border-b border-border flex items-center gap-3 px-6 shrink-0 bg-[var(--workbench-bg)]">
                <div className="flex-1 relative">
                    <Input
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        placeholder={t("settings.searchPlaceholder")}
                        className="h-8"
                    />
                </div>
                {onRequestClose && (
                    <Button
                        type="button"
                        variant="ghost"
                        size="icon"
                        aria-label="Close settings"
                        onClick={() => void requestClose()}
                    >
                        ×
                    </Button>
                )}
            </div>

            {resetAllError ? (
                <div className="shrink-0 px-6 pt-4">
                    <Alert data-settings-reset-all-error variant="destructive">
                        <VscError aria-hidden="true" />
                        <AlertDescription className="text-destructive">{resetAllError}</AlertDescription>
                    </Alert>
                </div>
            ) : null}

            <div className="flex min-h-0 flex-1 overflow-hidden max-[720px]:flex-col">
                {/* Sidebar Navigation */}
                <aside className="flex min-h-0 w-64 shrink-0 flex-col border-r border-border bg-[var(--sidebar-bg)] max-[720px]:h-12 max-[720px]:w-full max-[720px]:border-b max-[720px]:border-r-0">
                    <ScrollArea className="min-h-0 flex-1 pt-4 max-[720px]:pt-0" orientation="vertical">
                    <nav className="space-y-0.5 px-4 max-[720px]:flex max-[720px]:gap-1 max-[720px]:space-y-0 max-[720px]:overflow-x-auto max-[720px]:px-2 max-[720px]:py-1">
                        {visibleSections.map(section => (
                            <Button
                                type="button"
                                variant={activeSection === section.id ? "secondary" : "ghost"}
                                key={section.id}
                                onClick={() => void requestSection(section.id)}
                                className="w-full justify-start max-[720px]:w-auto max-[720px]:shrink-0"
                            >
                                {section.label}
                            </Button>
                        ))}
                    </nav>
                    </ScrollArea>
                </aside>

                {/* Main Content Area */}
                <main className="flex min-h-0 min-w-0 flex-1 flex-col">
                    <ScrollArea className="flex-1 min-h-0" orientation="vertical">
                    <div className="w-full max-w-4xl space-y-4 px-12 py-8 max-[720px]:px-4 max-[720px]:py-4">
                        {sectionResetError?.section === activeSection ? (
                            <Alert data-settings-section-reset-error variant="destructive">
                                <VscError aria-hidden="true" />
                                <AlertDescription className="text-destructive">
                                    {sectionResetError.message}
                                </AlertDescription>
                            </Alert>
                        ) : null}
                        {renderContent()}
                    </div>
                    </ScrollArea>
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

const ColorGroup: React.FC<{ title: string; children: React.ReactNode }> = ({ title, children }) => (
    <div className="mb-8">
        <h3 className="mb-4 text-[11px] font-bold uppercase tracking-widest text-muted-foreground opacity-70">
            {title}
        </h3>
        <div className="space-y-6">{children}</div>
    </div>
);

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
                            disabled={disabled}
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
