import { GraphProjectionChannelService } from "@/services/nodeSystem/graphProjectionChannelService";
import type { GraphProjectionSnapshotDto } from "@/shared/types/domain/graphProjectionChannel";

export async function recoverGraphProjectionSnapshot(
  projectInstanceId: string,
): Promise<GraphProjectionSnapshotDto> {
  const snapshot = await GraphProjectionChannelService.snapshot(projectInstanceId);
  if (snapshot.projectInstanceId !== projectInstanceId) {
    throw new Error("Recovered Graph Projection snapshot targets another project");
  }
  return snapshot;
}
