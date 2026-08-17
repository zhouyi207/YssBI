import { invokeCommand } from "@/services/ipc";

export type JuliaRuntimeState = "missing" | "ready" | "invalid";

export type JuliaWorkerEnvironmentState = "missing" | "ready" | "invalid";
export type JuliaWorkerProcessState = "stopped" | "starting" | "running" | "crashed";

export interface JuliaWorkerStatus {
  runtimeState: JuliaRuntimeState;
  environmentState: JuliaWorkerEnvironmentState;
  processState: JuliaWorkerProcessState;
  projectDir: string;
}

export interface JuliaRuntimeStatus {
  state: JuliaRuntimeState;
  version: string | null;
  installDir: string | null;
}

export class JuliaRuntimeService {
  static async getStatus(): Promise<JuliaRuntimeStatus> {
    return invokeCommand<JuliaRuntimeStatus>("get_julia_runtime_status");
  }

  static async getWorkerStatus(): Promise<JuliaWorkerStatus> {
    return invokeCommand<JuliaWorkerStatus>("get_julia_worker_status");
  }

  static async install(): Promise<JuliaRuntimeStatus> {
    return invokeCommand<JuliaRuntimeStatus>("install_julia_runtime");
  }
}
