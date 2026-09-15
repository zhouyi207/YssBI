import { useGraphEditingStore } from "./graphEditingStore";

export interface GraphDraftUiSnapshot {
  readonly saving: boolean;
}

export function useGraphEditingUi(graphPath: string): GraphDraftUiSnapshot {
  const saving = useGraphEditingStore((state) => state.sessions[graphPath]?.saving === true);
  return { saving };
}
