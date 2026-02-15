import { invoke } from "@tauri-apps/api/core";
import { Variable } from "@/shared/types/domain";

/**
 * 变量服务 - 统一的变量管理接口
 * 
 * 使用统一的 CRUD 接口处理全局和局部变量
 * 变量的作用域通过 Variable 对象中的 scope 字段区分
 */
export class VariableService {
    /**
     * 创建变量（统一接口）
     * @param variable 变量对象（包含 scope 信息）
     * @returns 创建后的变量 ID
     */
    static async createVariable(variable: Variable): Promise<string> {
        console.log('[VariableService.createVariable] Creating variable:', variable);
        const id = await invoke<string>("create_variable", { variable });
        console.log('[VariableService.createVariable] Variable created with ID:', id);
        return id;
    }

    /**
     * 获取变量（统一接口）
     * @param id 变量 ID
     * @returns 变量对象
     */
    static async getVariable(id: string): Promise<Variable> {
        console.log('[VariableService.getVariable] Getting variable:', id);
        const variable = await invoke<Variable>("get_variable", { id });
        console.log('[VariableService.getVariable] Variable retrieved:', variable);
        return variable;
    }

    /**
     * 更新变量（统一接口）
     * @param id 变量 ID
     * @param variable 更新后的变量对象
     */
    static async updateVariable(id: string, variable: Variable): Promise<void> {
        console.log('[VariableService.updateVariable] Updating variable:', id, variable);
        await invoke("update_variable", { id, variable });
        console.log('[VariableService.updateVariable] Variable updated successfully');
    }

    /**
     * 删除变量（统一接口）
     * @param id 变量 ID
     */
    static async deleteVariable(id: string): Promise<void> {
        console.log('[VariableService.deleteVariable] Deleting variable:', id);
        await invoke("delete_variable", { id });
        console.log('[VariableService.deleteVariable] Variable deleted successfully');
    }
}
