import { invoke } from "@tauri-apps/api/core";

export class WindowDataService {
  static async getWindowData(key: string): Promise<string | null> {
    return invoke<string | null>("get_window_data", { key });
  }

  static async getWindowSourceValue(key: string): Promise<string | null> {
    return invoke<string | null>("get_window_source_value", { key });
  }
}
