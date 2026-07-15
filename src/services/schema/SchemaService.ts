import type { EditorSchema } from "@/shared/types/domain/schema";
import { invoke } from "@tauri-apps/api/core";

export class SchemaService {
    static async getEditorSchema(): Promise<EditorSchema> {
        return invoke<EditorSchema>("get_editor_schema_command");
    }
}
