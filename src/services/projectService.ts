import { save, open } from "@tauri-apps/plugin-dialog";
import { writeTextFile, readTextFile } from "@tauri-apps/plugin-fs";
import { invoke } from "@tauri-apps/api/core";
import { TabState } from "../components/Editor/Store/useNodeStore";
import { SubGraphData, ProjectData } from "../components/Editor/Types/canvas";
import { serializeSubGraph, serializeProject, deserializeProject } from "../components/Editor/Utils/io";

export class ProjectService {
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

    static async saveProjectAs(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>
    ): Promise<string | null> {
        try {
            const project = serializeProject(globalVariables, events, functions, macros);
            const path = await save({ filters: [{ name: "JSON", extensions: ["json"] }] });
            if (path) {
                await writeTextFile(path, JSON.stringify(project, null, 2));
                return path;
            }
        } catch (e) {
            console.error(e);
            throw e;
        }
        return null;
    }

    static async saveProject(
        path: string,
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>
    ): Promise<void> {
        const project = serializeProject(globalVariables, events, functions, macros);
        await writeTextFile(path, JSON.stringify(project, null, 2));
    }

    static async loadProject(jsonContent?: string): Promise<{ project: ProjectData, path: string | null } | null> {
        try {
            let content = jsonContent;
            let path: string | null = null;

            if (!content) {
                const selected = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
                if (!selected || Array.isArray(selected)) return null;
                path = selected as string;
                content = await readTextFile(path);
            }

            if (!content) return null;

            const project = deserializeProject(content);
            return { project, path };
        } catch (e) {
            console.error(e);
            throw e;
        }
    }

    static async executeProject(
        globalVariables: Record<string, any>,
        events: Record<string, SubGraphData>,
        functions: Record<string, SubGraphData>,
        macros: Record<string, SubGraphData>
    ): Promise<string> {
        const project = serializeProject(globalVariables, events, functions, macros);
        const res: string = await invoke("execute_graph", { projectJson: JSON.stringify(project) });
        return res;
    }
}
