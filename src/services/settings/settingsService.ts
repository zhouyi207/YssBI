import { invoke } from "@tauri-apps/api/core";
import { AppSettings } from "@/shared/types/settings";
import { DEFAULT_SETTINGS } from "@/app/appConfig/default";


export class SettingsService {
    private static settingsCache: AppSettings | null = null;

    /**
     * 加载设置，如果文件不存在则返回默认设置
     */
    static async loadSettings(): Promise<AppSettings> {
        try {
            const settings = await invoke<AppSettings>("load_settings");
            this.settingsCache = settings;
            console.info("[SettingsService.loadSettings] Settings loaded successfully via backend");
            return settings;
        } catch (error) {
            console.error("[SettingsService.loadSettings] Error loading settings via backend:", error);
            this.settingsCache = { ...DEFAULT_SETTINGS };
            return this.settingsCache;
        }
    }

    /**
     * 保存设置到后端
     */
    static async saveSettings(settings: AppSettings): Promise<void> {
        try {
            await invoke("save_settings", { settings });
            this.settingsCache = settings;
            console.log("[SettingsService] Settings saved successfully via backend");
        } catch (error) {
            console.error("[SettingsService] Error saving settings via backend:", error);
            throw error;
        }
    }


    /**
     * 恢复默认设置
     */
    static async resetToDefaults(): Promise<AppSettings> {
        await this.saveSettings(DEFAULT_SETTINGS);
        return { ...DEFAULT_SETTINGS };
    }

    /**
     * 获取缓存的设置（同步）
     */
    static getCachedSettings(): AppSettings | null {
        return this.settingsCache;
    }


}
