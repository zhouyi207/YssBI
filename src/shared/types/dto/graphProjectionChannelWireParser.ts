import type {
  GraphProjectionChannelEventDto,
  GraphProjectionPublicationDto,
  GraphProjectionSnapshotDto,
  GraphProjectionSubscriptionDto,
} from "./graphProjectionChannel";
import type { ProjectionStatusDto } from "./editorMutation";
import { parseGraphProjectionReplacementDto } from "./editorMutationWireParser";
import { isGraphResourcePath } from "./editorProjectionGuards";

type UnknownRecord = Record<string, unknown>;

function isRecord(value: unknown): value is UnknownRecord {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function hasExactKeys(value: UnknownRecord, keys: readonly string[]): boolean {
  return (
    Object.keys(value).length === keys.length &&
    keys.every((key) => Object.prototype.hasOwnProperty.call(value, key))
  );
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === "string" && value.length > 0;
}

function isPositiveSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) > 0;
}

function isNonNegativeSafeInteger(value: unknown): value is number {
  return Number.isSafeInteger(value) && (value as number) >= 0;
}

function parseProjectionStatus(value: unknown): ProjectionStatusDto {
  if (!isRecord(value) || typeof value.status !== "string") {
    throw new Error("Graph Projection batch status is malformed");
  }
  if (
    value.status === "complete" &&
    hasExactKeys(value, ["status", "expectedGraphPaths"]) &&
    Array.isArray(value.expectedGraphPaths) &&
    value.expectedGraphPaths.every(isGraphResourcePath)
  ) {
    return { status: "complete", expectedGraphPaths: value.expectedGraphPaths };
  }
  if (
    value.status === "incomplete" &&
    hasExactKeys(value, ["status", "invalidatedGraphPaths"]) &&
    Array.isArray(value.invalidatedGraphPaths) &&
    value.invalidatedGraphPaths.every(isGraphResourcePath)
  ) {
    return { status: "incomplete", invalidatedGraphPaths: value.invalidatedGraphPaths };
  }
  throw new Error("Graph Projection batch status is malformed");
}

export function parseGraphProjectionPublicationDto(value: unknown): GraphProjectionPublicationDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "projectInstanceId",
      "graphSessionId",
      "graphPath",
      "requestGeneration",
      "replacement",
    ]) ||
    !isNonEmptyString(value.projectInstanceId) ||
    !isNonEmptyString(value.graphSessionId) ||
    !isGraphResourcePath(value.graphPath) ||
    !isPositiveSafeInteger(value.requestGeneration)
  ) {
    throw new Error("Graph Projection publication is malformed");
  }
  const replacement = parseGraphProjectionReplacementDto(value.replacement);
  if (replacement.graphPath !== value.graphPath) {
    throw new Error("Graph Projection publication targets another graph");
  }
  return {
    projectInstanceId: value.projectInstanceId,
    graphSessionId: value.graphSessionId,
    graphPath: value.graphPath,
    requestGeneration: value.requestGeneration,
    replacement,
  };
}

export function parseGraphProjectionChannelEventDto(
  value: unknown,
): GraphProjectionChannelEventDto {
  if (!isRecord(value) || typeof value.type !== "string") {
    throw new Error("Graph Projection event is malformed");
  }
  if (value.type === "projectionReplaced") {
    if (
      !hasExactKeys(value, [
        "type",
        "projectInstanceId",
        "graphSessionId",
        "graphPath",
        "requestGeneration",
        "replacement",
      ])
    ) {
      throw new Error("Graph Projection replacement event is malformed");
    }
    return {
      type: "projectionReplaced",
      ...parseGraphProjectionPublicationDto({
        projectInstanceId: value.projectInstanceId,
        graphSessionId: value.graphSessionId,
        graphPath: value.graphPath,
        requestGeneration: value.requestGeneration,
        replacement: value.replacement,
      }),
    };
  }
  if (value.type === "projectionBatchReplaced") {
    if (
      !hasExactKeys(value, [
        "type",
        "projectInstanceId",
        "publicationRevision",
        "replacements",
        "status",
      ]) ||
      !isNonEmptyString(value.projectInstanceId) ||
      !isNonNegativeSafeInteger(value.publicationRevision) ||
      !Array.isArray(value.replacements)
    ) {
      throw new Error("Graph Projection batch event is malformed");
    }
    const replacements = value.replacements.map(parseGraphProjectionPublicationDto);
    if (
      replacements.some((replacement) => replacement.projectInstanceId !== value.projectInstanceId)
    ) {
      throw new Error("Graph Projection batch contains another project");
    }
    return {
      type: "projectionBatchReplaced",
      projectInstanceId: value.projectInstanceId,
      publicationRevision: value.publicationRevision,
      replacements,
      status: parseProjectionStatus(value.status),
    };
  }
  if (
    value.type === "projectionInvalidated" &&
    hasExactKeys(value, [
      "type",
      "projectInstanceId",
      "graphSessionId",
      "graphPath",
      "requestGeneration",
      "reasonCode",
      "incidentId",
    ]) &&
    isNonEmptyString(value.projectInstanceId) &&
    isNonEmptyString(value.graphSessionId) &&
    isGraphResourcePath(value.graphPath) &&
    isPositiveSafeInteger(value.requestGeneration) &&
    isNonEmptyString(value.reasonCode) &&
    (value.incidentId === null || isNonEmptyString(value.incidentId))
  ) {
    return {
      type: "projectionInvalidated",
      projectInstanceId: value.projectInstanceId,
      graphSessionId: value.graphSessionId,
      graphPath: value.graphPath,
      requestGeneration: value.requestGeneration,
      reasonCode: value.reasonCode,
      incidentId: value.incidentId,
    };
  }
  throw new Error("Graph Projection event is malformed");
}

export function parseGraphProjectionSnapshotDto(value: unknown): GraphProjectionSnapshotDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, [
      "projectInstanceId",
      "streamId",
      "projections",
      "latestGenerationByGraph",
    ]) ||
    !isNonEmptyString(value.projectInstanceId) ||
    !isNonEmptyString(value.streamId) ||
    !Array.isArray(value.projections) ||
    !isRecord(value.latestGenerationByGraph)
  ) {
    throw new Error("Graph Projection snapshot is malformed");
  }
  const projections = value.projections.map(parseGraphProjectionPublicationDto);
  if (projections.some((projection) => projection.projectInstanceId !== value.projectInstanceId)) {
    throw new Error("Graph Projection snapshot contains another project");
  }
  const latestGenerationByGraph: Record<string, number> = {};
  for (const [graphPath, generation] of Object.entries(value.latestGenerationByGraph)) {
    if (!isGraphResourcePath(graphPath) || !isPositiveSafeInteger(generation)) {
      throw new Error("Graph Projection snapshot generation map is malformed");
    }
    latestGenerationByGraph[graphPath] = generation;
  }
  if (new Set(projections.map((projection) => projection.graphPath)).size !== projections.length) {
    throw new Error("Graph Projection snapshot contains duplicate graphs");
  }
  for (const projection of projections) {
    if (
      latestGenerationByGraph[projection.graphPath] === undefined ||
      projection.requestGeneration > latestGenerationByGraph[projection.graphPath]
    ) {
      throw new Error("Graph Projection snapshot generation is inconsistent");
    }
  }
  return {
    projectInstanceId: value.projectInstanceId,
    streamId: value.streamId,
    projections,
    latestGenerationByGraph,
  };
}

export function parseGraphProjectionSubscriptionDto(
  value: unknown,
): GraphProjectionSubscriptionDto {
  if (
    !isRecord(value) ||
    !hasExactKeys(value, ["subscriptionId", "snapshot"]) ||
    !isNonEmptyString(value.subscriptionId)
  ) {
    throw new Error("Graph Projection subscription is malformed");
  }
  return {
    subscriptionId: value.subscriptionId,
    snapshot: parseGraphProjectionSnapshotDto(value.snapshot),
  };
}
