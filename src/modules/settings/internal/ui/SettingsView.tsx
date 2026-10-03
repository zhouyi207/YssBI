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
import { useSettingsReset } from "@/features/application/settings/useSettingsReset";
import { Select } from "@/shared/ui";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Button } from "@/components/ui/button";
import { Switch } from "@/components/ui/switch";
import { Input } from "@/components/ui/input";
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog";
import type { AppLanguage } from "@/shared/types/settings";
import "./settings.css";

export function SettingsDialog({
  modalId,
  onClose,
}: {
  readonly modalId: string;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent
        aria-describedby={undefined}
        className="flex h-[min(720px,88dvh)] max-w-[min(1000px,92vw)] flex-col gap-0 rounded-md bg-[var(--workbench-bg)] p-0 motion-reduce:animate-none max-[720px]:h-[92dvh] max-[720px]:max-w-[96vw]"
      >
        <div className="settings-header">
          <div className="settings-title">
            <VscSettingsGear aria-hidden="true" />
            <DialogTitle className="text-xs font-medium normal-case tracking-normal">
              {t("settings.title")}
            </DialogTitle>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="settings-close"
            aria-label={t("settings.close")}
            title={t("settings.close")}
            onClick={onClose}
          >
            <VscClose aria-hidden="true" />
          </Button>
        </div>
        <div className="min-h-0 flex-1">
          <SettingsView modalId={modalId} />
        </div>
      </DialogContent>
    </Dialog>
  );
}

export const SettingsView: React.FC<{ readonly modalId: string }> = ({ modalId }) => {
  const { t } = useTranslation();
  const ai = useSettingsRead((s) => s.ai);
  const appearance = useSettingsRead((s) => s.appearance);
  const isLoading = useSettingsRead((s) => s.isLoading);
  const updateAi = settingsUi.updateAi;
  const updateAppearance = settingsUi.updateAppearance;
  const { resetSettings, isResetPending, isResetting, resetAllError, sectionResetError } =
    useSettingsReset(modalId);

  const [activeSection, setActiveSection] = useState("ai");
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

  if (isLoading) {
    return (
      <div className="settings-view">
        <div
          className="settings-loading flex min-h-0 flex-1 flex-col"
          role="status"
          aria-busy="true"
        >
          <div className="text-sm text-muted-foreground">{t("settings.loading")}</div>
          <div aria-hidden="true" className="settings-skeleton" />
          <div aria-hidden="true" className="settings-skeleton" />
          <div aria-hidden="true" className="settings-skeleton" />
        </div>
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
                  onClick={() => resetSettings("ai")}
                  disabled={isResetPending}
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
                  onClick={() => resetSettings("appearance")}
                  disabled={isResetPending}
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
                    updateAppearance({ language: val as AppLanguage });
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
      {resetAllError ? (
        <div className="shrink-0 px-6 pt-4">
          <Alert data-settings-reset-all-error variant="destructive">
            <VscError aria-hidden="true" />
            <AlertDescription className="text-destructive">{resetAllError}</AlertDescription>
          </Alert>
        </div>
      ) : null}

      <div className="settings-body">
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
            <Button
              type="button"
              variant="ghost"
              onClick={() => resetSettings("all")}
              disabled={isResetPending}
            >
              {isResetting ? t("common.restoring") : t("common.restoreAllDefaults")}
            </Button>
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
}

type SettingItemProps =
  | (SettingItemBase & {
      type: "checkbox";
      checked: boolean;
      onChange: (val: boolean) => void;
    })
  | (SettingItemBase & {
      type: "text" | "password";
      value: string;
      onChange: (val: string) => void;
    })
  | (SettingItemBase & {
      type: "select";
      value: string;
      options: Array<{ label: string; value: string }>;
      onChange: (val: string) => void;
    });

const SettingItem: React.FC<SettingItemProps> = (props) => {
  const { label, description, type, placeholder } = props;
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
            checked={props.checked}
            onCheckedChange={(value) => props.onChange(value === true)}
          />
        )}
        {(type === "text" || type === "password") && (
          <Input
            id={controlId}
            type={type}
            aria-describedby={`${controlId}-description`}
            value={props.value}
            onChange={(e) => props.onChange(e.target.value)}
            placeholder={placeholder}
            autoComplete={type === "password" ? "off" : undefined}
            className="settings-input"
          />
        )}
        {type === "select" && (
          <div className="w-full">
            <Select
              id={controlId}
              className="settings-input"
              options={props.options}
              value={props.value}
              onChange={(val) => props.onChange(val)}
            />
          </div>
        )}
      </div>
    </div>
  );
};
