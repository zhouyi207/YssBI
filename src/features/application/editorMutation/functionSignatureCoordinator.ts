import { currentProjectionLocale } from "@/features/application/graphProjection/projectionLocale";
import { hydrateGraphProjection } from "@/features/application/graphProjection/graphProjectionLifecycle";

import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import { captureRevisionedProjectCommandSnapshot } from "@/features/application/projectCommandContext";

import {
  ProjectPublicationError,
  projectPublicationCoordinator,
} from "./projectPublicationCoordinator";
import { dataTypeDisplay } from "@/shared/types/domain/dataType";
import type { FunctionSignaturePatch } from "@/shared/types";
import type {
  FunctionDocumentPatchDto,
  FunctionSignatureDto,
  MutationRequestDto,
  ResourceMutationResultDto,
} from "@/shared/types/domain/editorMutation";
import { FunctionMutationService } from "@/services/nodeSystem/functionMutationService";
import { isApplicationIpcErrorCode } from "@/features/application/errorReference";
import {
  isCurrentProjectIdentity,
  type ProjectIdentitySnapshot,
} from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { isGraphDraftSaving } from "@/features/core/graphDraft";

export interface ExecuteFunctionSignatureMutationInput {
  functionPath: string;
  locale: string;
  patch: FunctionSignaturePatch;
}

export interface FunctionSignatureCoordinatorDependencies {
  mutateSignature(
    projectInstanceId: string,
    functionPath: string,
    locale: string,
    request: MutationRequestDto<FunctionDocumentPatchDto>,
  ): Promise<ResourceMutationResultDto>;
  hydrateGraph(graphPath: string, locale: string): Promise<unknown>;
  refreshResourceIndex(): Promise<void>;
}

export type ExecuteFunctionSignatureMutationOutcome =
  | { status: "applied"; result: ResourceMutationResultDto }
  | { status: "stale"; result?: ResourceMutationResultDto }
  | { status: "conflict" };

let coordinatorEpoch = 0;

const defaultDependencies: FunctionSignatureCoordinatorDependencies = {
  mutateSignature: (projectInstanceId, functionPath, locale, request) =>
    FunctionMutationService.updateSignature(projectInstanceId, functionPath, locale, request),
  hydrateGraph: hydrateGraphProjection,
  refreshResourceIndex: () => projectPublicationCoordinator.refreshIndex(),
};

function isFunctionRevisionConflict(error: unknown): boolean {
  return isApplicationIpcErrorCode(error, "function_revision_conflict");
}

function buildSignature(
  before: FunctionSignatureDto,
  patch: FunctionSignaturePatch,
): FunctionSignatureDto {
  const parameters =
    patch.inputs === undefined
      ? before.parameters
      : patch.inputs
          .filter((pin) => pin.dataType != null)
          .map((pin) => ({
            id: pin.id,
            name: pin.name,
            type_name: dataTypeDisplay(pin.dataType!),
          }));
  const returnType =
    patch.outputs === undefined
      ? before.return_type
      : patch.outputs.find((pin) => pin.dataType != null)?.dataType;
  return {
    parameters,
    return_type:
      typeof returnType === "string" ? returnType : returnType ? dataTypeDisplay(returnType) : null,
  };
}

function validateDirectSignatureResult(
  request: MutationRequestDto<FunctionDocumentPatchDto>,
  result: ResourceMutationResultDto,
): string | undefined {
  if (result.deltas.some((delta) => delta.causedBy !== request.operationId)) {
    return "operation correlation does not match the pending request";
  }
  const signatureDelta = result.deltas.find(
    (delta) => delta.resource.kind === "function" && delta.resource.key === request.resource.key,
  );
  if (!signatureDelta) return "function signature delta is missing";
  if (signatureDelta.fromRevision !== request.baseRevision) {
    return "function signature revision does not match the request";
  }
  if (
    signatureDelta.payload.kind !== "function" ||
    JSON.stringify(signatureDelta.payload.patch) !== JSON.stringify(request.payload)
  ) {
    return "function signature delta does not match the request payload";
  }
  return undefined;
}

async function hydrateAuthoritativeState(
  graphPaths: Iterable<string>,
  locale: string,
  dependencies: FunctionSignatureCoordinatorDependencies,
  identity: ProjectIdentitySnapshot,
): Promise<void> {
  await dependencies.refreshResourceIndex();
  if (!isCurrentProjectIdentity(identity)) return;
  await Promise.all(
    [...new Set(graphPaths)].map((graphPath) => dependencies.hydrateGraph(graphPath, locale)),
  );
  if (!isCurrentProjectIdentity(identity)) return;
}

export async function executeFunctionSignatureMutation(
  input: ExecuteFunctionSignatureMutationInput,
  overrides: Partial<FunctionSignatureCoordinatorDependencies> = {},
): Promise<ExecuteFunctionSignatureMutationOutcome> {
  if (isGraphDraftSaving(input.functionPath)) return { status: "stale" };
  const dependencies = { ...defaultDependencies, ...overrides };
  const { context, captured: meta } = captureRevisionedProjectCommandSnapshot(
    () => useGraphMetaStore.getState().graphs[input.functionPath],
  );
  if (meta?.type !== "function" || meta.functionRevision == null || !meta.functionSignature) {
    throw new Error(`function signature resource '${input.functionPath}' is not hydrated`);
  }

  const requestPatch: FunctionDocumentPatchDto = {
    before: meta.functionSignature,
    after: buildSignature(meta.functionSignature, input.patch),
  };
  const request: MutationRequestDto<FunctionDocumentPatchDto> = {
    resource: { kind: "function", key: input.functionPath },
    baseRevision: meta.functionRevision,
    operationId: context.operationId,
    payload: requestPatch,
  };
  const epoch = coordinatorEpoch;
  const identity: ProjectIdentitySnapshot = {
    projectInstanceId: context.projectInstanceId,
    epoch: context.projectEpoch,
  };
  let result: ResourceMutationResultDto;
  try {
    result = await dependencies.mutateSignature(
      identity.projectInstanceId,
      input.functionPath,
      input.locale,
      request,
    );
    if (!isCurrentProjectIdentity(identity)) return { status: "stale", result };
  } catch (error) {
    if (
      !isCurrentProjectIdentity(identity) ||
      isApplicationIpcErrorCode(error, "stale_project_lifecycle")
    )
      return { status: "stale" };
    if (epoch !== coordinatorEpoch) return { status: "stale" };
    if (!isFunctionRevisionConflict(error)) throw error;
    await hydrateAuthoritativeState([input.functionPath], input.locale, dependencies, identity);
    if (!isCurrentProjectIdentity(identity) || epoch !== coordinatorEpoch) {
      return { status: "stale" };
    }
    return { status: "conflict" };
  }

  if (epoch !== coordinatorEpoch) return { status: "stale", result };
  try {
    await projectPublicationCoordinator.submit({
      result,
      fallbackPaths: [input.functionPath],
      validate: (candidate) => validateDirectSignatureResult(request, candidate),
    });
  } catch (error) {
    if (error instanceof ProjectPublicationError && error.code === "stale_project_lifecycle") {
      return { status: "stale", result };
    }
    throw error;
  }
  if (epoch !== coordinatorEpoch) return { status: "stale", result };
  return { status: "applied", result };
}

export function commitFunctionSignature(
  functionPath: string,
  patch: FunctionSignaturePatch,
): Promise<ExecuteFunctionSignatureMutationOutcome> {
  return executeFunctionSignatureMutation({
    functionPath,
    locale: currentProjectionLocale(),
    patch,
  });
}

export function resetFunctionSignatureCoordinator(): void {
  coordinatorEpoch += 1;
}
