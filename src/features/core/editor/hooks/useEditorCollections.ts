/**
 * 获取 events、functions、dataframes 集合
 * Explorer 图列表来自 ResourceStore 快照。
 */

import { useMemo } from "react";
import { useDatabaseStore } from "@/features/core/dataStore";
import { useGraphResourcesByKind } from "@/features/core/resource";
import { useFunctionCatalog } from "./useFunctionCatalog";
import type { EditorCollections } from "../editorCollections";

export function useEditorCollections(): EditorCollections {
  const events = useGraphResourcesByKind("event");
  const functions = useFunctionCatalog();
  const dataframes = useDatabaseStore((s) => s.databases);

  return useMemo(() => ({ events, functions, dataframes }), [events, functions, dataframes]);
}
