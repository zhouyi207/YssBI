import { invokeCommand } from "@/services/ipc";

export interface ParseAtRequest {
  param_names: string[];
  at_spec: string;
}

export interface ParseAtResponse {
  values: Record<string, number>;
}

export async function parseAtValues(req: ParseAtRequest): Promise<ParseAtResponse> {
  return await invokeCommand<ParseAtResponse>("parse_at_values", { req });
}
