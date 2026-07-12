/**
 * 编辑器资源集合类型（`useEditorCollections` 与 canvas drop / palette 共用）
 */

import type { GraphResourceRecord } from '@/features/core/resource/resourceSelectors';
import type { DatabaseRecord } from '@/shared/types/dto/database';
import type { Variable } from '@/shared/types/domain/variable';
import type { FunctionResourceView } from '@/features/core/resource/functionResourceView';

export type EditorEvents = GraphResourceRecord;
export type EditorFunctions = Record<string, FunctionResourceView>;
export type EditorVariables = Record<string, Variable>;
export type EditorDataframes = Record<string, DatabaseRecord>;

export interface EditorCollections {
  events: EditorEvents;
  functions: EditorFunctions;
  variables: EditorVariables;
  dataframes: EditorDataframes;
}
