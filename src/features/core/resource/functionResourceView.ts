/**
 * 函数资源视图：ResourceStore（名称）+ graphMetaStore（签名）的单点组装。
 *
 * | 字段 | 权威 Store |
 * | --- | --- |
 * | name | ResourceStore |
 * | functionInputs / functionOutputs | graphMetaStore |
 * | nodes / pins / connections | GraphDataStore |
 *
 * UI / palette / Detail 只读此视图或 `useFunctionCatalog`，禁止第四处手写合并。
 */

import type { FunctionSignaturePin } from "@/shared/types";
import type { GraphMeta } from "@/features/core/dataStore/graphMetaStore";
import type { GraphResourceRecord } from "./resourceSelectors";

export interface FunctionResourceView {
  id: string;
  name: string;
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
}

export function buildFunctionResourceView(
  id: string,
  resource: Pick<{ id: string; name: string }, "id" | "name">,
  meta?: Pick<GraphMeta, "functionInputs" | "functionOutputs">,
): FunctionResourceView {
  return {
    id,
    name: resource.name,
    functionInputs: meta?.functionInputs ?? [],
    functionOutputs: meta?.functionOutputs ?? [],
  };
}

export function buildFunctionResourceCatalog(
  resources: GraphResourceRecord,
  metaGraphs: Record<string, GraphMeta>,
): Record<string, FunctionResourceView> {
  const result: Record<string, FunctionResourceView> = {};
  for (const [id, resource] of Object.entries(resources)) {
    result[id] = buildFunctionResourceView(id, resource, metaGraphs[id]);
  }
  return result;
}
