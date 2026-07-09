import type { AppLanguage } from "./LanguageSettings";

export interface AppearanceSettings {
    colorTheme: string;
    language: AppLanguage;
    activityBarPosition: string;
    panelPosition: string;
    smoothScroll: boolean;
}