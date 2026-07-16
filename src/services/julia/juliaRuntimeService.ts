import { invoke } from "@tauri-apps/api/core";

export type JuliaRuntimeState = "missing" | "ready" | "invalid";

export interface JuliaRuntimeStatus {
  state: JuliaRuntimeState;
  version: string | null;
  installDir: string | null;
  message: string | null;
}

export class JuliaRuntimeService {
  static async getStatus(): Promise<JuliaRuntimeStatus> {
    return invoke<JuliaRuntimeStatus>("get_julia_runtime_status");
  }

  static async install(): Promise<JuliaRuntimeStatus> {
    return invoke<JuliaRuntimeStatus>("install_julia_runtime");
  }
}
