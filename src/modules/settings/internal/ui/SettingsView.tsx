import React, { useEffect, useMemo, useState } from "react";
import {
  VscClose,
  VscColorMode,
  VscError,
  VscSearch,
  VscSettingsGear,
  VscSparkle,
} from "react-icons/vsc";
import { useTranslation } from "react-i18next";
import { useSettingsRead } from "@/features/core/settings/read";
import { settingsUi } from "@/features/core/settings/ui";
import { ui } from "@/features/core/ui/ui";
import { Select } from "@/shared/ui";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { i18n, type AppLanguage } from "@/app/i18n";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import "./settings.css";

interface SettingsViewProps {
  onRequestClose?: () => void;
}

export const SettingsView: React.FC<SettingsViewProps> = ({ onRequestClose }) => {
  const { t } = useTranslation();
  const ai = useSettingsRead((s) => s.ai);
  const appearance = useSettingsRead((s) => s.appearance);
  const isLoading = useSettingsRead((s) => s.isLoading);
  const updateAi = settingsUi.updateAi;
  const updateAppearance = settingsUi.updateAppearance;
  const resetAllToDefaults = settingsUi.resetAllToDefaults;
  const resetAiToDefaults = settingsUi.resetAiToDefaults;
  const resetAppearanceToDefaults = settingsUi.resetAppearanceToDefaults;

  const [activeSection, setActiveSection] = useState("ai");
  const [isResetting, setIsResetting] = useState(false);
  const [resetAllError, setResetAllError] = useState<string | null>(null);
  const [sectionResetError, setSectionResetError] = useState<{
    section: string;
    message: string;
  } | null>(null);
  const [searchQuery, setSearchQuery] = useState("");

  const sections = [
    { id: "ai", label: t("settings.sections.ai"), icon: VscSparkle },
    { id: "appearance", label: t("settings.sections.appearance"), icon: VscColorMode },
  ];

  const visibleSections = useMemo(() => {
    const query = searchQuery.trim().toLowerCase();
    if (!query) return sections;
    return sections.filter((section) => section.label.toLowerCase().includes(query));
  }, [sections, searchQuery]);

  useEffect(() => {
    if (
      visibleSections.length > 0 &&
      !visibleSections.some((section) => section.id === activeSection)
    ) {
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

  const titleBarStyleOptions = [
    { label: t("settings.options.titleBarCustom"), value: "custom" },
    { label: t("settings.options.titleBarNative"), value: "native" },
  ];

  const handleResetAll = async () => {
    const confirmed = await ui.confirm({
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
      setResetAllError(
        t("settings.restoreAllFailed", {
          error: formatInlineUserError(error, t),
        }),
      );
    } finally {
      setIsResetting(false);
    }
  };

  const handleResetSection = async (section: string) => {
    const sectionNames: Record<string, string> = {
      ai: t("settings.sections.ai"),
      appearance: t("settings.sections.appearance"),
    };

    const sectionName = sectionNames[section] || section;
    const confirmed = await ui.confirm({
      title: t("settings.confirmResetTitle"),
      message: t("settings.confirmResetMessage", { section: sectionName }),
      type: "danger",
      confirmText: t("common.restoreDefaults"),
    });
    if (!confirmed) return;

    setIsResetting(true);
    setSectionResetError((current) => (current?.section === section ? null : current));
    try {
      switch (section) {
        case "ai":
          await resetAiToDefaults();
          break;
        case "appearance":
          await resetAppearanceToDefaults();
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
      <div className="settings-view settings-loading" role="status" aria-busy="true">
        <div className="text-sm text-muted-foreground">{t("settings.loading")}</div>
        <div aria-hidden="true" className="settings-skeleton" />
        <div aria-hidden="true" className="settings-skeleton" />
        <div aria-hidden="true" className="settings-skeleton" />
      </div>
    );
  }

  const renderContent = () => {
    switch (activeSection) {
      case "ai":
        return (
          <div className="space-y-8">
            <div>
              <div className="settings-section-heading">
                <div>
                  <h2>{t("settings.sections.ai")}</h2>
                  <p>{t("settings.sectionDescriptions.ai")}</p>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => handleResetSection("ai")}
                  disabled={isResetting}
                >
                  {t("common.restoreDefaults")}
                </Button>
              </div>
              <div className="settings-fields">
                <SettingItem
                  label={t("settings.labels.openAiModel")}
                  description={t("settings.descriptions.openAiModel")}
                  type="text"
                  value={ai.openAiModel}
                  onChange={(value) => updateAi({ openAiModel: value })}
                  placeholder="gpt-4o-mini"
                />
                <SettingItem
                  label={t("settings.labels.openAiBaseUrl")}
                  description={t("settings.descriptions.openAiBaseUrl")}
                  type="text"
                  value={ai.openAiBaseUrl}
                  onChange={(value) => updateAi({ openAiBaseUrl: value })}
                  placeholder="https://api.openai.com/v1"
                />
                <SettingItem
                  label={t("settings.labels.openAiApiKey")}
                  description={t("settings.descriptions.openAiApiKey")}
                  type="password"
                  value={ai.openAiApiKey}
                  onChange={(value) => updateAi({ openAiApiKey: value })}
                  placeholder="sk-..."
                />
              </div>
            </div>
          </div>
        );
      case "appearance":
        return (
          <div className="space-y-8">
            <div>
              <div className="settings-section-heading">
                <div>
                  <h2>{t("settings.sections.appearance")}</h2>
                  <p>{t("settings.sectionDescriptions.appearance")}</p>
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="sm"
                  onClick={() => handleResetSection("appearance")}
                  disabled={isResetting}
                >
                  {t("common.restoreDefaults")}
                </Button>
              </div>
              <div className="settings-fields">
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
      default:
        return null;
    }
  };

  return (
    <div className="settings-view">
      <div className="settings-header">
        <div className="settings-title">
          <VscSettingsGear aria-hidden="true" />
          <h1>{t("settings.title")}</h1>
        </div>
        {onRequestClose && (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            aria-label={t("settings.close")}
            className="settings-close"
            onClick={onRequestClose}
          >
            <VscClose aria-hidden="true" />
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
        <aside className="settings-sidebar">
          <div className="settings-search">
            <VscSearch aria-hidden="true" />
            <Input
              type="text"
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              placeholder={t("settings.searchPlaceholder")}
              aria-label={t("settings.searchPlaceholder")}
              className="h-7 pl-7"
            />
          </div>
          <ScrollArea className="min-h-0 flex-1" orientation="vertical">
            <nav aria-label={t("settings.title")} className="settings-nav">
              {visibleSections.map((section) => (
                <Button
                  type="button"
                  variant="ghost"
                  key={section.id}
                  onClick={() => setActiveSection(section.id)}
                  aria-current={activeSection === section.id ? "page" : undefined}
                  className="settings-nav-item"
                >
                  <section.icon aria-hidden="true" />
                  {section.label}
                </Button>
              ))}
            </nav>
          </ScrollArea>
          <div className="settings-footer">
            <Button type="button" variant="ghost" onClick={handleResetAll} disabled={isResetting}>
              {isResetting ? t("common.restoring") : t("common.restoreAllDefaults")}
            </Button>
            <span>{t("settings.applyHint")}</span>
          </div>
        </aside>

        {/* Main Content Area */}
        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          <ScrollArea className="flex-1 min-h-0" orientation="vertical">
            <div className="settings-content">
              {visibleSections.length > 0 && sectionResetError?.section === activeSection ? (
                <Alert data-settings-section-reset-error variant="destructive">
                  <VscError aria-hidden="true" />
                  <AlertDescription className="text-destructive">
                    {sectionResetError.message}
                  </AlertDescription>
                </Alert>
              ) : null}
              {visibleSections.length > 0 ? (
                renderContent()
              ) : (
                <div className="settings-empty" role="status">
                  <VscSearch aria-hidden="true" />
                  <h2>{t("settings.noResults")}</h2>
                  <p>{t("settings.noResultsHint")}</p>
                  <Button variant="outline" onClick={() => setSearchQuery("")}>
                    {t("settings.clearSearch")}
                  </Button>
                </div>
              )}
            </div>
          </ScrollArea>
        </main>
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
      type: "password";
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
    });

const SettingItem: React.FC<SettingItemProps> = (props) => {
  const { label, description, type, placeholder, disabled } = props;
  const controlId = React.useId();

  return (
    <div className="settings-field">
      <label htmlFor={controlId} className="mb-1.5 block text-sm font-medium text-foreground">
        {label}
      </label>
      <div
        id={`${controlId}-description`}
        className="text-xs text-muted-foreground mb-3 leading-relaxed max-w-2xl"
      >
        {description}
      </div>

      <div className="settings-control">
        {type === "checkbox" && (
          <Switch
            className="settings-switch"
            id={controlId}
            aria-describedby={`${controlId}-description`}
            disabled={disabled}
            checked={props.checked ?? false}
            onCheckedChange={(value) => props.onChange?.(value === true)}
          />
        )}
        {type === "text" && (
          <Input
            id={controlId}
            type="text"
            aria-describedby={`${controlId}-description`}
            value={props.value ?? props.defaultValue ?? ""}
            onChange={(e) => props.onChange?.(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            className="settings-input"
          />
        )}
        {type === "password" && (
          <Input
            id={controlId}
            type="password"
            aria-describedby={`${controlId}-description`}
            value={props.value ?? props.defaultValue ?? ""}
            onChange={(e) => props.onChange?.(e.target.value)}
            placeholder={placeholder}
            disabled={disabled}
            autoComplete="off"
            className="settings-input"
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
          <div className="w-full">
            <Select
              id={controlId}
              className="settings-input"
              options={props.options || []}
              value={props.value || props.options?.[0]?.value || ""}
              onChange={(val) => props.onChange?.(val)}
              disabled={disabled}
            />
          </div>
        )}
      </div>
    </div>
  );
};
