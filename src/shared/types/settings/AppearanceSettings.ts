import type { AppLanguage } from "./LanguageSettings";

/** VS Code `window.titleBarStyle`: custom frameless chrome vs OS-native decorations. */
export type TitleBarStyle = "custom" | "native";

export interface AppearanceSettings {
    colorTheme: string;
    lastLightColorTheme: string;
    lastDarkColorTheme: string;
    language: AppLanguage;

    smoothScroll: boolean;
    titleBarStyle: TitleBarStyle;
}
