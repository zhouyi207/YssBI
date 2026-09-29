import { create } from "zustand";
import type { GraphPath, FunctionSignaturePin } from "@/shared/types";
import type { FunctionSignatureDto } from "@/shared/types/domain/editorMutation";

/** 函数签名投影（名称见 ResourceStore，图体见 GraphProjectionStore）。 */
export interface GraphMeta {
  type: "event_graph" | "function_graph";
  functionRevision?: number;
  functionSignature?: FunctionSignatureDto;
  functionInputs?: FunctionSignaturePin[];
  functionOutputs?: FunctionSignaturePin[];
}

interface GraphMetaStore {
  graphs: Record<GraphPath, GraphMeta>;

  clear(): void;
}

export const useGraphMetaStore = create<GraphMetaStore>((set) => ({
  graphs: {},

  clear: () =>
    set({
      graphs: {},
    }),
}));
