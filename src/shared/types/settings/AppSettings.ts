import { ThemeSettings } from "./ThemeSettings";
import { AppearanceSettings } from "./AppearanceSettings";
import { AiSettings } from "./AiSettings";

/**
 * 持久化的客户端设置：AI 提供方、主题和外观。
 * 窗口几何状态独立保存于后端 `window_state.json`，由 `WindowStateService` 读写，
 * 不再混入 AppSettings；详见 `src/services/window/windowStateService.ts`。
 */
export interface AppSettings {
  ai: AiSettings;
  theme: ThemeSettings;
  appearance: AppearanceSettings;
}

// 深度部分类型，允许嵌套属性也是可选的
export interface PartialAppSettings {
  ai?: Partial<AiSettings>;
  theme?: Partial<ThemeSettings>;
  appearance?: Partial<AppearanceSettings>;
}
