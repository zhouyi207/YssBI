import type { DocEdit, DocSnapshot } from "@/shared/types/domain/doc";
import { isDocPath } from "@/shared/types/domain/doc";
import { parseFileSnapshot, createFileContentService } from "@/services/project/fileContentService";
export function parseDocSnapshot(value: unknown): DocSnapshot {
  return parseFileSnapshot(value, "doc", isDocPath, (value) => {
    if (typeof value !== "string") throw new Error("Invalid Markdown content");
    return value;
  });
}
export const DocService = createFileContentService<DocSnapshot, DocEdit>(
  "read_project_doc",
  "edit_project_doc",
  parseDocSnapshot,
);
