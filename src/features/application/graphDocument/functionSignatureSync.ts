import { useGraphMetaStore } from "@/features/core/dataStore/graphMetaStore";
import type { FunctionSignatureDto } from "@/shared/types/domain/editorMutation";
import type { FunctionEditorProjectionDto } from "@/shared/types/domain/editorProjection";
import type { ProjectGraphIndexRow } from "@/shared/types/domain/project";
import type { PreparedFunctionDeltaInstall } from "@/features/application/editorMutation/projectPublicationCoordinator";

export function installFunctionEditorProjection(
  graphPath: string,
  signature: FunctionSignatureDto,
  projection: FunctionEditorProjectionDto,
): PreparedFunctionDeltaInstall {
  return {
    graphPath,
    revision: projection.functionRevision,
    signature: structuredClone(signature),
    functionInputs: structuredClone(projection.inputs),
    functionOutputs: structuredClone(projection.outputs),
  };
}

function hasExactFunctionProjection(
  existing: ReturnType<typeof useGraphMetaStore.getState>["graphs"][string],
  signature: FunctionSignatureDto,
  projection: FunctionEditorProjectionDto,
): boolean {
  return (
    JSON.stringify({
      signature: existing.functionSignature,
      inputs: existing.functionInputs,
      outputs: existing.functionOutputs,
    }) ===
    JSON.stringify({
      signature,
      inputs: projection.inputs,
      outputs: projection.outputs,
    })
  );
}

/** 项目打开 / 索引刷新：从 `getProjectIndex` 的函数行 hydrate 签名表（与后端索引层对齐）。 */
export function hydrateFunctionSignaturesFromProjectIndex(graphs: ProjectGraphIndexRow[]): void {
  const graphMetaStore = useGraphMetaStore.getState();
  for (const row of graphs) {
    if (row.type !== "function") continue;
    const existing = graphMetaStore.graphs[row.path];
    if (existing?.functionRevision != null) {
      if (existing.functionRevision > row.functionRevision) continue;
      if (
        existing.functionRevision === row.functionRevision &&
        hasExactFunctionProjection(existing, row.functionSignature, row.functionEditorProjection)
      )
        continue;
    }
    const install = installFunctionEditorProjection(
      row.path,
      row.functionSignature,
      row.functionEditorProjection,
    );
    const patch = {
      functionRevision: install.revision,
      functionSignature: install.signature,
      functionInputs: [...install.functionInputs],
      functionOutputs: [...install.functionOutputs],
    };
    if (existing) {
      graphMetaStore.updateGraph(row.path, patch);
      continue;
    }
    graphMetaStore.addGraph({
      path: row.path,
      name: row.name,
      type: "function",
      ...patch,
    });
  }
}
