    // ==================== 兼容旧接口 ====================

    /**
     * Synchronizes the live state from the node store (tabs) back to the persistent collections.
     */
    static syncStoreToCollections(
        tabs: Record<string, TabState>,
        currentEvents: Record<string, SubGraphData>,
        currentFunctions: Record<string, SubGraphData>,
        currentMacros: Record<string, SubGraphData>
    ): {
        nextEvents: Record<string, SubGraphData>;
        nextFunctions: Record<string, SubGraphData>;
        nextMacros: Record<string, SubGraphData>;
        changed: boolean;
    } {
        const nextEvents = { ...currentEvents };
        const nextFunctions = { ...currentFunctions };
        const nextMacros = { ...currentMacros };
        let changed = false;

        Object.keys(tabs).forEach((id) => {
            const { nodes: liveNodes, variables: liveVars } = tabs[id];

            // Identify which collection the tab belongs to
            let targetCollection: Record<string, SubGraphData> | null = null;
            if (nextEvents[id]) targetCollection = nextEvents;
            else if (nextFunctions[id]) targetCollection = nextFunctions;
            else if (nextMacros[id]) targetCollection = nextMacros;

            if (!targetCollection) return;

            const existing = targetCollection[id];
            if (!existing) return;

            // 调试：打印 existing 的 type 字段
            console.log(`[syncStoreToCollections] id=${id}, existing.type=${existing.type}`);

            const subGraph = serializeSubGraph(
                id,
                existing.name,
                existing.type as any,
                liveNodes,
                existing.canvas,
                liveVars,
                existing.inputs || [],
                existing.outputs || []
            );

            // Check for changes (rudimentary check, or just overwrite as intended)
            // For now we overwrite to ensure consistency, optimization can be added if needed
            targetCollection[id] = { ...existing, ...subGraph };
            changed = true;
        });

        return { nextEvents, nextFunctions, nextMacros, changed };
    }

    /**
     * 构建项目数据对象（内存中）
     */
    static buildProjectData(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): ProjectData {
        return {
            globalVariables,
            events,
            functions,
            macros,
            dataframes,
            metadata: {
                exportTime: new Date().toISOString(),
                appVersion: "0.1.0",
            },
        };
    }

    /**
     * 另存为项目文件（弹出文件选择对话框）- 兼容旧接口
     */
    static async saveProjectAs(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<string | null> {
        try {
            const path = await save({ filters: [{ name: "YssBI Project", extensions: ["json"] }] });
            if (path) {
                const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
                // 调用后端保存项目
                await invoke("save_project", {
                    path,
                    projectJson: JSON.stringify(project)
                });
                return path;
            }
        } catch (e) {
            console.error("Failed to save project:", e);
            throw e;
        }
        return null;
    }

    /**
     * 保存项目到指定路径 - 兼容旧接口
     */
    static async saveProject(
        path: string,
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>,
        dataframes: Record<string, any> = {}
    ): Promise<void> {
        const project = this.buildProjectData(globalVariables, events, functions, macros, dataframes);
        // 调用后端保存项目
        await invoke("save_project", {
            path,
            projectJson: JSON.stringify(project)
        });
    }

    /**
     * 加载项目文件 - 兼容旧接口
     */
    static async loadProject(): Promise<{ project: ProjectData, path: string | null } | null> {
        try {
            // 弹出文件选择对话框
            const selected = await open({
                multiple: false,
                filters: [{ name: "YssBI Project", extensions: ["json"] }]
            });
            if (!selected || Array.isArray(selected)) return null;

            const path = selected as string;
            // 调用后端加载项目
            const project: ProjectData = await invoke("load_project", { path });
            return { project, path };
        } catch (e) {
            console.error("Failed to load project:", e);
            throw e;
        }
    }