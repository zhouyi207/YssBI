import { NodeDefinition, TypeSystemSnapshot } from "@/shared/types";
import { invoke } from "@tauri-apps/api/core";

export interface EditorSchemaDTO {
    nodeDefinitions: NodeDefinition[];
    typeSystem: TypeSystemSnapshot;
}

export class SchemaService {
    static async getEditorSchema(): Promise<EditorSchemaDTO> {
        return invoke<EditorSchemaDTO>("get_editor_schema_command");
    }
}
