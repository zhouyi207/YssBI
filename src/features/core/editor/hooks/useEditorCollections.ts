/**
 * 获取 eventGraphs、functionGraphs、dataframes 集合
 * Explorer 图列表来自 ResourceStore 快照。
 */

import { useMemo } from "react";
import { useDatabaseStore } from "@/features/core/dataStore";
import { useGraphResourcesByKind } from "@/features/core/resource";
import { useFunctionCatalog } from "./useFunctionCatalog";
import type { EditorCollections } from "../editorCollections";

export function useEditorCollections(): EditorCollections {
  const eventGraphs = useGraphResourcesByKind("event_graph");
  const functionGraphs = useFunctionCatalog();
  const dataframes = useDatabaseStore((s) => s.databases);

  return useMemo(
    () => ({ eventGraphs, functionGraphs, dataframes }),
    [eventGraphs, functionGraphs, dataframes],
  );
}
