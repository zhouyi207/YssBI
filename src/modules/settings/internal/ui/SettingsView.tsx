import { LanguageModelSettings } from "./LanguageModelSettings";
import { KnowledgeSettings } from "./KnowledgeSettings";
import { SettingsField } from "./SettingsField";
import { SettingsPage } from "./SettingsPage";
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
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
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
        className="flex h-[min(760px,88dvh)] max-w-[min(1000px,92vw)] flex-col gap-0 bg-background p-0 motion-reduce:animate-none max-[720px]:h-[92dvh] max-[720px]:max-w-[96vw]"
      >
        <div className="settings-header">
          <div className="settings-title">
            <VscSettingsGear aria-hidden="true" />
            <DialogTitle className="text-sm font-semibold normal-case tracking-normal">
              {t("settings.title")}
            </DialogTitle>
          </div>
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="text-muted-foreground"
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
  const appearance = useSettingsRead((s) => s.appearance);
  const isLoading = useSettingsRead((s) => s.isLoading);
  const updateAppearance = settingsUi.updateAppearance;
  const { resetSettings, isResetPending, isResetting, resetAllError, sectionResetError } =
    useSettingsReset(modalId);

  const [activeSection, setActiveSection] = useState("ai");
  const [searchQuery, setSearchQuery] = useState("");

  const sections = [
    { id: "ai", label: t("settings.sections.ai"), icon: VscSparkle },
    { id: "knowledge", label: t("settings.knowledge.title"), icon: VscSearch },
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

  const currentSectionError =
    visibleSections.length > 0 && sectionResetError?.section === activeSection
      ? sectionResetError.message
      : null;
  const pageNotice =
    resetAllError || currentSectionError ? (
      <>
        {resetAllError && (
          <Alert data-settings-reset-all-error variant="destructive">
            <VscError aria-hidden="true" />
            <AlertDescription className="text-destructive">{resetAllError}</AlertDescription>
          </Alert>
        )}
        {currentSectionError && (
          <Alert data-settings-section-reset-error variant="destructive">
            <VscError aria-hidden="true" />
            <AlertDescription className="text-destructive">{currentSectionError}</AlertDescription>
          </Alert>
        )}
      </>
    ) : null;

  const renderContent = () => {
    switch (activeSection) {
      case "ai":
        return <LanguageModelSettings notice={pageNotice} />;
      case "knowledge":
        return <KnowledgeSettings notice={pageNotice} />;
      case "appearance":
        return (
          <SettingsPage
            breadcrumbs={[{ label: t("settings.sections.appearance") }]}
            actions={
              <Button
                type="button"
                variant="ghost"
                size="sm"
                onClick={() => resetSettings("appearance")}
                disabled={isResetPending}
              >
                {t("common.restoreDefaults")}
              </Button>
            }
            notice={pageNotice}
          >
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
          </SettingsPage>
        );
      default:
        return null;
    }
  };

  return (
    <div className="settings-view">
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
              className="pl-9"
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
              {isResetting ? t("common.restoring") : t("settings.restorePreferences")}
            </Button>
          </div>
        </aside>

        {/* Main Content Area */}
        <main className="flex min-h-0 min-w-0 flex-1 flex-col">
          {visibleSections.length > 0 ? (
            renderContent()
          ) : (
            <ScrollArea className="flex-1 min-h-0" orientation="vertical">
              <div className="settings-content">
                {pageNotice}
                <div className="settings-empty" role="status">
                  <VscSearch aria-hidden="true" />
                  <h2>{t("settings.noResults")}</h2>
                  <p>{t("settings.noResultsHint")}</p>
                  <Button variant="outline" onClick={() => setSearchQuery("")}>
                    {t("settings.clearSearch")}
                  </Button>
                </div>
              </div>
            </ScrollArea>
          )}
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
    <SettingsField htmlFor={controlId} label={label} description={description}>
      {type === "checkbox" && (
        <Switch
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
        />
      )}
      {type === "select" && (
        <Select value={props.value} onValueChange={props.onChange}>
          <SelectTrigger id={controlId} aria-describedby={`${controlId}-description`}>
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {props.options.map((option) => (
              <SelectItem key={option.value} value={option.value}>
                {option.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      )}
    </SettingsField>
  );
};
