import { useActiveGraphContext } from "@/features/application/editor/editorGroupContext";
import { resourceKey } from "@/features/core/resource";
import { useResourceRead } from "@/features/core/resource/read";
export interface ActiveProjectGraph {
  path: string;
  kind: "event" | "function";
  name: string;
}

export function useActiveProjectGraph(): ActiveProjectGraph | null {
  const activeGraph = useActiveGraphContext();

  return useResourceRead((snapshot) => {
    if (!activeGraph) return null;
    const resource =
      snapshot.resources[resourceKey({ id: activeGraph.graphPath, kind: activeGraph.kind })];
    return resource
      ? {
          path: activeGraph.graphPath,
          kind: activeGraph.kind,
          name: resource.name,
        }
      : null;
  });
}
