import { useProjectIOStore } from "./projectIOStore";

export function useGraphLoadStatus(graphPath: string) {
  return useProjectIOStore((state) => state.graphLoadStatus[graphPath]);
}
