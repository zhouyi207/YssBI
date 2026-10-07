import { AppearanceSettings } from "./AppearanceSettings";

/**
 * 持久化的客户端设置：外观偏好（含主题选择，不含独立颜色值）。
 * 原生窗口几何状态由 Rust Window State 插件独立保存，不混入 AppSettings。
 */
export interface AppSettings {
  appearance: AppearanceSettings;
}

// 深度部分类型，允许嵌套属性也是可选的
export interface PartialAppSettings {
  appearance?: Partial<AppearanceSettings>;
}
