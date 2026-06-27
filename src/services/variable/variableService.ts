import { invoke } from '@tauri-apps/api/core';
import type { Variable } from '@/shared/types/domain';
import { dataTypeToBackend } from '@/shared/types/dto/dataType';
import { dataValueToBackend } from '@/shared/types/dto/dataValue';
import { normalizeVariableFromBackend } from '@/shared/types/dto/variable';

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
  static async createVariable(variable: Omit<Variable, 'id'>): Promise<string> {
    const id = await invoke<string>('create_variable', {
      name: variable.name,
      dataType: dataTypeToBackend(variable.dataType),
      dataValue: dataValueToBackend(variable.dataValue),
      description: variable.description,
      scope: variable.scope,
      tags: variable.tags,
    });
    return id;
  }

  /**
   * 获取变量
   */
  static async getVariable(variableId: string): Promise<Variable> {
    const raw = await invoke<Record<string, unknown>>('get_variable', { variableId });
    return normalizeVariableFromBackend(raw as Parameters<typeof normalizeVariableFromBackend>[0]);
  }

  /**
   * 更新变量（部分字段）
   */
  static async updateVariable(id: string, patch: Partial<Variable>): Promise<Variable> {
    const raw = await invoke<Record<string, unknown>>('update_variable', {
      variableId: id,
      name: patch.name ?? null,
      dataType: patch.dataType ? dataTypeToBackend(patch.dataType) : null,
      dataValue: patch.dataValue !== undefined ? dataValueToBackend(patch.dataValue) : null,
      description: patch.description ?? null,
      tags: patch.tags ?? null,
    });
    return normalizeVariableFromBackend(raw as Parameters<typeof normalizeVariableFromBackend>[0]);
  }

  /**
   * 删除变量
   */
  static async deleteVariable(variableId: string): Promise<void> {
    await invoke('delete_variable', { variableId });
  }
}
