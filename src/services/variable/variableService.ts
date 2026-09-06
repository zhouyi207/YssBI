import { invokeCommand } from "@/services/ipc";
import type { Variable } from "@/shared/types/domain";
import { dataTypeToBackend } from "@/shared/types/dto/dataType";
import { dataValueToBackend } from "@/shared/types/dto/dataValue";
import { normalizeVariableFromBackend } from "@/shared/types/domain/variable";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";
import { isUuid } from "@/shared/types/dto/editorProjectionGuards";
import { parseResourceMutationResultDto } from "@/shared/types/dto/resourceMutationResultWireParser";

export interface VariableMutationReceipt {
  variableId: string;
  mutation: ResourceMutationResultDto;
}

function parseVariableMutationReceipt(response: unknown): VariableMutationReceipt {
  if (
    typeof response !== "object" ||
    response === null ||
    Object.keys(response).length !== 2 ||
    !("variableId" in response) ||
    !isUuid(response.variableId) ||
    !("mutation" in response)
  ) {
    throw new Error("variable mutation receipt is malformed");
  }
  return {
    variableId: response.variableId,
    mutation: parseResourceMutationResultDto(response.mutation),
  };
}

export class VariableService {
  static async createVariable(
    projectInstanceId: string,
    operationId: string,
    expectedCollectionRevision: number,
    variable: Omit<Variable, "id" | "revision">,
  ): Promise<VariableMutationReceipt> {
    const response: unknown = await invokeCommand("create_variable", {
      name: variable.name,
      dataType: dataTypeToBackend(variable.dataType),
      dataValue: dataValueToBackend(variable.dataValue),
      description: variable.description,
      scope: variable.scope,
      tags: variable.tags,
      projectInstanceId,
      expectedCollectionRevision,
      operationId,
    });
    return parseVariableMutationReceipt(response);
  }

  static async getVariable(projectInstanceId: string, variableId: string): Promise<Variable> {
    const raw = await invokeCommand<Record<string, unknown>>("get_variable", {
      projectInstanceId,
      variableId,
    });
    return normalizeVariableFromBackend(raw as Parameters<typeof normalizeVariableFromBackend>[0]);
  }

  static async updateVariable(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    variableId: string,
    patch: Partial<Variable>,
  ): Promise<VariableMutationReceipt> {
    const response: unknown = await invokeCommand("update_variable", {
      variableId,
      name: patch.name ?? null,
      dataType: patch.dataType ? dataTypeToBackend(patch.dataType) : null,
      dataValue: patch.dataValue !== undefined ? dataValueToBackend(patch.dataValue) : null,
      description: patch.description ?? null,
      tags: patch.tags ?? null,
      projectInstanceId,
      expectedRevision,
      operationId,
    });
    return parseVariableMutationReceipt(response);
  }

  static async deleteVariable(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    variableId: string,
  ): Promise<VariableMutationReceipt> {
    const response: unknown = await invokeCommand("delete_variable", {
      projectInstanceId,
      operationId,
      expectedRevision,
      variableId,
    });
    return parseVariableMutationReceipt(response);
  }
}
