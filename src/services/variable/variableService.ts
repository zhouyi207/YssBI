import { invokeCommand } from "@/services/ipc";
import type { Variable } from "@/shared/types/domain";
import { dataTypeToBackend } from "@/shared/types/dto/dataType";
import { dataValueToBackend } from "@/shared/types/dto/dataValue";
import { normalizeVariableFromBackend } from "@/shared/types/domain/variable";
import type { ResourceMutationResultDto } from "@/shared/types/dto/editorMutation";

export interface VariableMutationCommandResult {
  variableId: string;
  variable: Variable | null;
  result: ResourceMutationResultDto | null;
}

type VariableMutationWireResult = {
  variableId: string;
  variable: Record<string, unknown> | null;
  result: ResourceMutationResultDto | null;
};

function normalizeCommandResult(raw: VariableMutationWireResult): VariableMutationCommandResult {
  return {
    variableId: raw.variableId,
    variable: raw.variable
      ? normalizeVariableFromBackend(
          raw.variable as Parameters<typeof normalizeVariableFromBackend>[0],
        )
      : null,
    result: raw.result,
  };
}

/**
 * 变量服务 - 统一的变量管理接口
 * 与后端 command_variable 对应
 */
export class VariableService {
  /**
   * 创建变量
   * @param variable 变量对象（id 由后端分配）
   * @returns 创建后的变量 ID
   */
  static async createVariable(
    projectInstanceId: string,
    operationId: string,
    expectedCollectionRevision: number,
    variable: Omit<Variable, "id" | "revision">,
  ): Promise<VariableMutationCommandResult> {
    const raw = await invokeCommand<VariableMutationWireResult>("create_variable", {
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
    return normalizeCommandResult(raw);
  }

  /**
   * 获取变量
   */
  static async getVariable(projectInstanceId: string, variableId: string): Promise<Variable> {
    const raw = await invokeCommand<Record<string, unknown>>("get_variable", {
      projectInstanceId,
      variableId,
    });
    return normalizeVariableFromBackend(raw as Parameters<typeof normalizeVariableFromBackend>[0]);
  }

  /**
   * 更新变量（部分字段）
   */
  static async updateVariable(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    id: string,
    patch: Partial<Variable>,
  ): Promise<VariableMutationCommandResult> {
    const raw = await invokeCommand<VariableMutationWireResult>("update_variable", {
      variableId: id,
      name: patch.name ?? null,
      dataType: patch.dataType ? dataTypeToBackend(patch.dataType) : null,
      dataValue: patch.dataValue !== undefined ? dataValueToBackend(patch.dataValue) : null,
      description: patch.description ?? null,
      tags: patch.tags ?? null,
      projectInstanceId,
      expectedRevision,
      operationId,
    });
    return normalizeCommandResult(raw);
  }

  /**
   * 删除变量
   */
  static async deleteVariable(
    projectInstanceId: string,
    operationId: string,
    expectedRevision: number,
    variableId: string,
  ): Promise<VariableMutationCommandResult> {
    const raw = await invokeCommand<VariableMutationWireResult>("delete_variable", {
      projectInstanceId,
      operationId,
      expectedRevision,
      variableId,
    });
    return normalizeCommandResult(raw);
  }
}
