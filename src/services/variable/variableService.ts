  // ==================== Global Variables CRUD ====================

    static async getGlobalVariables(): Promise<Record<string, VariableDefinition>> {
        return await invoke("get_global_variables");
    }

    static async getGlobalVariable(id: string): Promise<VariableDefinition | null> {
        return await invoke("get_global_variable", { id });
    }

    /**
     * 统一创建变量（后端生成 ID）
     * @param subgraphId 子图 ID（可选，null 为全局）
     * @param name 变量名称建议
     * @param dataType 数据类型
     */
    static async createVariable(
        subgraphId: string | null,
        name?: string,
        dataType?: string
    ): Promise<VariableDefinition> {
        return await invoke("create_variable", { subgraphId, name, dataType });
    }

    static async createGlobalVariable(id: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("create_global_variable", { id, data });
    }

    static async updateGlobalVariable(id: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("update_global_variable", { id, data });
    }

    static async deleteGlobalVariable(id: string): Promise<void> {
        await invoke("delete_global_variable", { id });
    }

    // ==================== Local Variables CRUD ====================

    static async getLocalVariables(subgraphId: string): Promise<Record<string, VariableDefinition>> {
        return await invoke("get_local_variables", { subgraphId });
    }

    static async createLocalVariable(subgraphId: string, variableId: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("create_local_variable", { subgraphId, variableId, data });
    }

    static async updateLocalVariable(subgraphId: string, variableId: string, data: VariableDefinition): Promise<VariableDefinition> {
        return await invoke("update_local_variable", { subgraphId, variableId, data });
    }

    static async deleteLocalVariable(subgraphId: string, variableId: string): Promise<void> {
        await invoke("delete_local_variable", { subgraphId, variableId });
    }
