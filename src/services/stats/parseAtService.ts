import { invoke } from "@tauri-apps/api/core";

export interface ParseAtRequest {
  param_names: string[];
  at_spec: string;
}

export interface ParseAtResponse {
  values: Record<string, number>;
}

export async function parseAtValues(req: ParseAtRequest): Promise<ParseAtResponse> {
  return await invoke<ParseAtResponse>("parse_at_values", { req });
}
