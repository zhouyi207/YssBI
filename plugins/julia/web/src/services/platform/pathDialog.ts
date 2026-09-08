import { request } from "@/sdk";
import type { PlatformOutcome } from "./platformTypes";
export function savePathDialog(options: {
  title?: string;
  defaultPath?: string;
  filters?: { name: string; extensions: string[] }[];
}): Promise<PlatformOutcome<string | null>> {
  return request("system.save_file", options);
}
