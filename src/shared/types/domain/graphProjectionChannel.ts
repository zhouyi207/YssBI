import type { GraphProjectionReplacementDto } from "./editorProjection";
import type { ProjectionStatusDto } from "./editorMutation";

export interface GraphProjectionPublicationDto {
  projectInstanceId: string;
  graphSessionId: string;
  graphPath: string;
  requestGeneration: number;
  replacement: GraphProjectionReplacementDto;
}

export type GraphProjectionChannelEventDto =
  | ({ type: "projectionReplaced" } & GraphProjectionPublicationDto)
  | {
      type: "projectionBatchReplaced";
      projectInstanceId: string;
      publicationRevision: number;
      replacements: GraphProjectionPublicationDto[];
      status: ProjectionStatusDto;
    }
  | {
      type: "projectionInvalidated";
      projectInstanceId: string;
      graphSessionId: string;
      graphPath: string;
      requestGeneration: number;
      reasonCode: string;
      incidentId: string | null;
    };

export interface GraphProjectionSnapshotDto {
  projectInstanceId: string;
  streamId: string;
  projections: GraphProjectionPublicationDto[];
  latestGenerationByGraph: Record<string, number>;
}

export interface GraphProjectionSubscriptionDto {
  subscriptionId: string;
  snapshot: GraphProjectionSnapshotDto;
}
