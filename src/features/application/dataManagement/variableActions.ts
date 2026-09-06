import type { Variable, VariableScope } from "@/shared/types/domain";
import { DEFAULT_VARIABLE_NAME } from "@/shared/constants/defaultResourceNames";
import {
  dataTypeFromKey,
  getDefaultValue,
  isVariableDataTypeAllowed,
} from "@/shared/types/domain/dataType";
import { dataValueFromRaw } from "@/shared/types/domain/dataValue";
import { useVariableStore } from "@/features/core/dataStore/variableStore";
import { ProjectLifecycleError } from "@/features/core/projectLifecycle/projectLifecycleAuthority";
import { VariableService } from "@/services/variable/variableService";
import { isApplicationIpcErrorCode } from "@/features/application/errorReference";
import { logger } from "@/features/application/observability/appLogger";
import {
  captureRevisionedProjectCommandSnapshot,
  type ProjectCommandContext,
} from "@/features/application/projectCommandContext";
import { projectPublicationCoordinator } from "@/features/application/editorMutation/projectPublicationCoordinator";

function isStaleProjectLifecycleError(error: unknown): boolean {
  return (
    error instanceof ProjectLifecycleError ||
    isApplicationIpcErrorCode(error, "stale_project_lifecycle")
  );
}

function resolveVariableScope(
  isGlobal: boolean,
  activeGraphPath: string | null,
  graphType: "event" | "function" | undefined,
): VariableScope {
  if (isGlobal || !activeGraphPath) return { type: "global" };
  const scopeType = graphType ?? "event";
  return scopeType === "function"
    ? { type: "function", functionPath: activeGraphPath }
    : { type: "event", eventPath: activeGraphPath };
}

export async function createVariableAction(params: {
  name?: string;
  dataTypeKey?: string;
  isGlobal?: boolean;
  activeGraphPath: string | null;
  graphType?: "event" | "function";
}): Promise<string | null> {
  let context: ProjectCommandContext | undefined;
  try {
    const baseName = params.name || DEFAULT_VARIABLE_NAME;
    const dataType = dataTypeFromKey(params.dataTypeKey ?? "Int64");
    if (!isVariableDataTypeAllowed(dataType)) return null;
    const snapshot = captureRevisionedProjectCommandSnapshot(
      (): Omit<Variable, "id" | "revision"> => ({
        name: baseName,
        dataType,
        dataValue: dataValueFromRaw(getDefaultValue(dataType), dataType),
        description: "",
        scope: resolveVariableScope(
          Boolean(params.isGlobal),
          params.activeGraphPath,
          params.graphType,
        ),
        tags: [],
      }),
    );
    context = snapshot.context;
    const variable = snapshot.captured;

    const receipt = await VariableService.createVariable(
      context.projectInstanceId,
      context.operationId,
      context.publicationRevision,
      variable,
    );
    if (!context.isCurrent()) return null;
    await projectPublicationCoordinator.submit({ result: receipt.mutation });
    if (!context.isCurrent()) return null;
    return receipt.variableId;
  } catch (error) {
    if (isStaleProjectLifecycleError(error) || (context && !context.isCurrent())) return null;
    logger.data.error("Failed to create variable: " + String(error), "VariableActions");
    return null;
  }
}

export async function updateVariableAction(
  variableId: string,
  patch: Partial<Variable>,
): Promise<Variable | null> {
  let context: ProjectCommandContext | undefined;
  try {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => {
      const variableState = useVariableStore.getState();
      return {
        previous: variableState.variables[variableId],
        expectedRevision: variableState.revisions[variableId],
      };
    });
    context = snapshot.context;
    const { previous, expectedRevision } = snapshot.captured;
    if (!previous || expectedRevision == null) return null;

    if (patch.dataType && !isVariableDataTypeAllowed(patch.dataType)) return null;

    const receipt = await VariableService.updateVariable(
      context.projectInstanceId,
      context.operationId,
      expectedRevision,
      variableId,
      patch,
    );
    if (!context.isCurrent()) return null;
    await projectPublicationCoordinator.submit({ result: receipt.mutation });
    if (!context.isCurrent()) return null;
    return useVariableStore.getState().variables[variableId] ?? null;
  } catch (error) {
    if (isStaleProjectLifecycleError(error) || (context && !context.isCurrent())) return null;
    logger.data.error("Failed to update variable in backend: " + String(error), "VariableActions");
    return null;
  }
}

export async function deleteVariableAction(variableId: string): Promise<boolean> {
  let context: ProjectCommandContext | undefined;
  try {
    const snapshot = captureRevisionedProjectCommandSnapshot(() => {
      const variableState = useVariableStore.getState();
      return {
        previous: variableState.variables[variableId],
        expectedRevision: variableState.revisions[variableId],
      };
    });
    context = snapshot.context;
    const { previous, expectedRevision } = snapshot.captured;
    if (!previous || expectedRevision == null) return false;

    const receipt = await VariableService.deleteVariable(
      context.projectInstanceId,
      context.operationId,
      expectedRevision,
      variableId,
    );
    if (!context.isCurrent()) return false;
    await projectPublicationCoordinator.submit({ result: receipt.mutation });
    return context.isCurrent();
  } catch (error) {
    if (isStaleProjectLifecycleError(error) || (context && !context.isCurrent())) return false;
    logger.data.error("Failed to delete variable in backend: " + String(error), "VariableActions");
    return false;
  }
}

export async function renameVariableAction(variableId: string, name: string): Promise<boolean> {
  const variableName = name.trim();
  if (!variableName) return false;
  const result = await updateVariableAction(variableId, { name: variableName });
  return result !== null;
}
