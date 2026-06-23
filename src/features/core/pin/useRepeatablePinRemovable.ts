import { useShallow } from "zustand/react/shallow";
import { useGraphDataStore } from "@/features/core/dataStore";
import { useNodeRegistryStore } from "@/features/core/nodeRegister/useNodeRegistryStore";
import { canRemoveRepeatablePin } from "./repeatablePinUtils";

/** Live removability from store + node registry (avoids stale layout props after +/- pin). */
export function useRepeatablePinRemovable(nodeId: string, pinId: string): boolean {
  const nodeType = useGraphDataStore((s) => s.nodes[nodeId]?.nodeType);
  const nodeDef = useNodeRegistryStore((s) =>
    nodeType ? s.definitions.get(nodeType) : undefined
  );
  const pinsOnNode = useGraphDataStore(
    useShallow((s) => {
      const ids = s.nodePins[nodeId] ?? [];
      return ids.map((id) => s.pins[id]).filter(Boolean);
    })
  );
  const pin = pinsOnNode.find((p) => p.id === pinId);
  if (!pin || !nodeDef) return false;
  return canRemoveRepeatablePin(pin, nodeDef, pinsOnNode);
}
