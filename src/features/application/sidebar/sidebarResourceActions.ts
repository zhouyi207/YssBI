import { logger } from "@/utils/frontendLogger";

import { ProjectService } from "@/services/project/projectService";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { revealPath } from "@/services/platform/opener";

import { captureProjectCommandContext } from "@/features/application/projectCommandContext";

export type RevealProjectResourceRequest = {
  readonly kind: import("@/shared/types/domain/resource").ResourceKind;
  readonly resourceId: string;
};

export async function revealProjectResourceInExplorer(
  request: RevealProjectResourceRequest,
): Promise<void> {
  const context = captureProjectCommandContext();
  try {
    const path = await ProjectService.getProjectResourcePath(context.projectInstanceId, request);
    if (!context.isCurrent()) return;
    const result = await revealPath(path);
    if (!result.ok) throw new Error(result.failure.code);
    if (!context.isCurrent()) return;
  } catch (error) {
    if (!context.isCurrent()) return;
    const ipcError = normalizeApplicationIpcError(error);
    logger.app.error(
      `Failed to reveal project resource code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
      "SidebarResourceActions",
    );
    throw error;
  }
}
