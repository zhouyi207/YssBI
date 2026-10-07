/**
 * 函数资源视图：ResourceStore（名称）+ graphMeta（签名）的单点组装。
 *
 * | 字段 | 权威 Store |
 * | --- | --- |
 * | name | ResourceStore |
 * | functionInputs / functionOutputs | graphMeta |
 * | nodes / pins / connections | ResourceStore |
 *
 * 函数详情复用此视图，名称与签名的组装不在消费者中重复实现。
 */

import type { FunctionSignaturePin } from "@/shared/types";
import type { GraphMeta } from "@/features/core/dataStore/graphMeta";

export interface FunctionResourceView {
  id: string;
  name: string;
  functionInputs: FunctionSignaturePin[];
  functionOutputs: FunctionSignaturePin[];
}

export function buildFunctionResourceView(
  resource: { id: string; name: string },
  meta?: Pick<GraphMeta, "functionInputs" | "functionOutputs">,
): FunctionResourceView {
  return {
    id: resource.id,
    name: resource.name,
    functionInputs: meta?.functionInputs ?? [],
    functionOutputs: meta?.functionOutputs ?? [],
  };
}
