import { invoke } from "@tauri-apps/api/core";

export type JuliaRuntimeState = "missing" | "ready" | "invalid";

export type JuliaWorkerEnvironmentState = "missing" | "ready" | "invalid";
export type JuliaWorkerProcessState = "stopped" | "starting" | "running" | "crashed";

export interface JuliaWorkerStatus {
  runtimeState: JuliaRuntimeState;
  environmentState: JuliaWorkerEnvironmentState;
  processState: JuliaWorkerProcessState;
  projectDir: string;
  message: string | null;
}

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

  static async getWorkerStatus(): Promise<JuliaWorkerStatus> {
    return invoke<JuliaWorkerStatus>("get_julia_worker_status");
  }

  static async install(): Promise<JuliaRuntimeStatus> {
    return invoke<JuliaRuntimeStatus>("install_julia_runtime");
  }
}
