import { useGraphDraftStore } from "./graphDraftStore";

export interface GraphDraftUiSnapshot {
  readonly saving: boolean;
}

export function useGraphDraftUi(graphPath: string): GraphDraftUiSnapshot {
  const saving = useGraphDraftStore((state) => state.sessions[graphPath]?.saving === true);
  return { saving };
}
