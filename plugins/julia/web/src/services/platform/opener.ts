import { request } from "@/sdk";
import type { PlatformOutcome } from "./platformTypes";
export function revealPath(path: string): Promise<PlatformOutcome<void>> {
  return request("system.reveal_artifact", { path });
}
