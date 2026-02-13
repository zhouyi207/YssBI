// ==================== 执行 ====================

    /**
     * 执行当前项目（从状态管理器获取数据）
     */
    static async execute(): Promise<string[]> {
        return await invoke("execute_graph");
    }

    /**
     * 执行指定的项目数据
     */
    static async executeProject(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<string> {
        const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
        // 直接使用 project，字段名已经匹配
        const backendData = {
            globalVariables: project.globalVariables,
            events: project.events,
            functions: project.functions,
            macros: project.macros,
            dataframes: project.dataframes,
            metadata: project.metadata,
        };
        const res: string[] = await invoke("execute_project", { data: backendData });
        return res.join("\n");
    }