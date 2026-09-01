import { ThemeSettings } from "./ThemeSettings";
import { EditorSettings } from "./EditorSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { ProjectSettings } from "./ProjectSettings";

/**
 * 持久化的客户端 UI 设置（主题/编辑器/外观/项目）。
 * 窗口几何状态独立保存于后端 `window_state.json`，由 `WindowStateService` 读写，
 * 不再混入 AppSettings；详见 `src/services/window/windowStateService.ts`。
 */
export interface AppSettings {
  theme: ThemeSettings;
  editor: EditorSettings;
  appearance: AppearanceSettings;
  project: ProjectSettings;
}

// 深度部分类型，允许嵌套属性也是可选的
export interface PartialAppSettings {
  theme?: Partial<ThemeSettings>;
  editor?: Partial<EditorSettings>;
  appearance?: Partial<AppearanceSettings>;
  project?: Partial<ProjectSettings>;
}
