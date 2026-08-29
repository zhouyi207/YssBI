import { useTranslation } from 'react-i18next';
import { VscGithub, VscSettingsGear } from 'react-icons/vsc';
import { i18n, type AppLanguage } from '@/app/i18n';
import { APP_LINKS } from '@/shared/config-default';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select';
import { getRememberedColorTheme } from '@/features/application/settings/colorThemePresets';
import {
  openExternalUrlWithDialog,
  useCurrentWindowActions,
  useCustomTitleBar,
} from '@/features/application/window';
import { useProjectProjection } from '@/features/application/project/projectProjection';
import { useSettingsRead } from '@/features/core/settings/read';
import { settingsUi } from '@/features/core/settings/ui';
import { BrandLockup } from '@/shared/ui/BrandMark';
import { ToolbarIconButton } from '@/shared/ui/ToolbarIconButton';
import { WindowChromeControls } from '@/shared/ui/WindowChromeControls';
import { WindowMenuBar } from '@/shared/ui/WindowChrome';

export function ProjectPickerTitleBar({
  onGoEditor,
  onOpenSettings,
}: {
  onGoEditor: () => void;
  onOpenSettings: () => void;
}) {
  const { t } = useTranslation();
  const currentPath = useProjectProjection().currentPath;
  const themeMode = useSettingsRead((state) => state.theme.mode ?? 'dark');
  const appearance = useSettingsRead((state) => state.appearance);
  const updateAppearance = settingsUi.updateAppearance;
  const isLightTheme = themeMode === 'light';
  const windowActions = useCurrentWindowActions();
  const customChrome = useCustomTitleBar();
  const toggleThemeMode = () => {
    const nextMode = isLightTheme ? 'dark' : 'light';
    updateAppearance({
      colorTheme: getRememberedColorTheme(nextMode, appearance.lastLightColorTheme, appearance.lastDarkColorTheme),
    });
  };

  return (
    <WindowMenuBar
      customChrome={customChrome}
      toolbar={
        <>
          {currentPath ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              onClick={onGoEditor}
              className="mr-1 h-7 self-center px-3 text-muted-foreground hover:text-foreground"
            >
              {t('projectPicker.backToEditor')}
            </Button>
          ) : null}
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={() => void openExternalUrlWithDialog(APP_LINKS.repository, t)}
            className="self-center text-muted-foreground"
            tooltip={t('menubar.githubRepository')}
            aria-label={t('menubar.githubRepository')}
          >
            <VscGithub size={16} />
          </ToolbarIconButton>
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={toggleThemeMode}
            className="self-center text-muted-foreground"
            tooltip={isLightTheme ? t('menubar.switchToDark') : t('menubar.switchToLight')}
            aria-label={isLightTheme ? t('menubar.switchToDark') : t('menubar.switchToLight')}
          >
            {isLightTheme ? (
              <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M21 12.8A8.5 8.5 0 1111.2 3a7 7 0 009.8 9.8z" />
              </svg>
            ) : (
              <svg className="size-4" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M12 3v2m0 14v2m9-9h-2M5 12H3m15.36-6.36-1.42 1.42M7.06 16.94l-1.42 1.42m12.72 0-1.42-1.42M7.06 7.06 5.64 5.64" />
                <circle cx="12" cy="12" r="4" strokeWidth={2} />
              </svg>
            )}
          </ToolbarIconButton>
          <ToolbarIconButton
            type="button"
            variant="ghost"
            size="icon-lg"
            onClick={onOpenSettings}
            className="self-center text-muted-foreground"
            tooltip={t('menubar.settings')}
            aria-label={t('menubar.settings')}
          >
            <VscSettingsGear size={14} />
          </ToolbarIconButton>
        </>
      }
      windowActions={(
        <WindowChromeControls
          maximized={windowActions.maximized}
          minimize={windowActions.minimize}
          toggleMaximize={windowActions.toggleMaximize}
          close={windowActions.close}
        />
      )}
    >
      <BrandLockup className="pointer-events-none self-center px-4" />
      <div className="pointer-events-none my-2.5 flex items-center border-l border-[var(--strong-border)] pl-4 font-heading text-[11px] font-medium tracking-wide text-muted-foreground">
        {t('projectPicker.title')}
      </div>
    </WindowMenuBar>
  );
}

export function ProjectSettingsDialog({
  open,
  onOpenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}) {
  const { t } = useTranslation();
  const language = useSettingsRead((state) => state.appearance.language);
  const updateAppearance = settingsUi.updateAppearance;
  const languageOptions = [
    { label: t('language.zhCN'), value: 'zh-CN' },
    { label: t('language.enUS'), value: 'en-US' },
  ];

  const handleLanguageChange = (value: string) => {
    const nextLanguage = value as AppLanguage;
    updateAppearance({ language: nextLanguage });
    void i18n.changeLanguage(nextLanguage);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="w-[min(420px,92vw)]">
        <DialogHeader>
          <DialogTitle>{t('menubar.settings')}</DialogTitle>
          <DialogDescription>{t('projectPicker.settingsDescription')}</DialogDescription>
        </DialogHeader>
        <div className="space-y-3 px-6 pb-5">
          <div className="space-y-1.5">
            <Label htmlFor="project-picker-language" className="text-sm font-medium text-foreground">
              {t('settings.labels.language')}
            </Label>
            <p className="text-xs leading-relaxed text-muted-foreground">
              {t('settings.descriptions.language')}
            </p>
          </div>
          <Select value={language} onValueChange={handleLanguageChange}>
            <SelectTrigger id="project-picker-language" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {languageOptions.map((option) => (
                <SelectItem key={option.value} value={option.value}>
                  {option.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>
        <DialogFooter>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>
            {t('common.close')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
