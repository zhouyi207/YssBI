/**
 * 编辑器资源集合类型（`useEditorCollections` 与 canvas drop / palette 共用）
 */

import type { GraphResourceRecord } from "@/features/core/resource/resourceSelectors";
import type { DatabaseRecord } from "@/shared/types/domain/database";
import type { FunctionResourceView } from "@/features/core/resource/functionResourceView";

export type EditorEventGraphs = GraphResourceRecord;
export type EditorFunctionGraphs = Record<string, FunctionResourceView>;
export type EditorDataframes = Record<string, DatabaseRecord>;

export interface EditorCollections {
  eventGraphs: EditorEventGraphs;
  functionGraphs: EditorFunctionGraphs;
  dataframes: EditorDataframes;
}
