import { logger } from "@/features/application/observability/appLogger";

import { ProjectService } from "@/services/project/projectService";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";
import { revealPath } from "@/services/platform/opener";

import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { renameResource } from "@/features/application/resource/resourceActions";

export type RevealProjectResourceRequest = {
  readonly kind: "graph" | "database" | "chart";
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
    const ipcError = normalizeApplicationIpcError("reveal_project_resource", error);
    logger.app.error(
      `Failed to reveal project resource code=${ipcError.code} incidentId=${ipcError.incidentId ?? "none"}`,
      "SidebarResourceActions",
    );
    throw error;
  }
}

export async function renameChartResource(chartPath: string, nextName: string): Promise<void> {
  await renameResource({ id: chartPath, kind: "chart" }, nextName);
}
