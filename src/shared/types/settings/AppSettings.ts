import { ThemeSettings } from "./ThemeSettings";
import { EditorSettings } from "./EditorSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { ProjectSettings } from "./ProjectSettings";
import { WindowSettings } from "./WindowSettings";

export interface AppSettings {
    theme: ThemeSettings;
    editor: EditorSettings;
    appearance: AppearanceSettings;
    project: ProjectSettings;
    window: WindowSettings;
}

// 深度部分类型，允许嵌套属性也是可选的
export interface PartialAppSettings {
    theme?: Partial<ThemeSettings>;
    editor?: Partial<EditorSettings>;
    appearance?: Partial<AppearanceSettings>;
    project?: Partial<ProjectSettings>;
    window?: Partial<WindowSettings>;
}