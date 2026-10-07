import type { ExecutionDemandDto } from "@/shared/types/domain/executionDemand";

export interface GraphContextMenuActions {
  runNode:
    | ((
        nodeId: string,
        mode: Extract<ExecutionDemandDto, { type: "node" }>["mode"],
      ) => Promise<void>)
    | null;
  selectNode: (nodeId: string, groupId?: string) => void;
  copyNode: (nodeId: string) => void;
  cutNode: (nodeId: string) => Promise<boolean | undefined>;
  duplicateNode: (nodeId: string) => Promise<boolean | undefined>;
  deleteNode: (nodeId: string) => Promise<boolean | undefined>;
  breakAllNodeLinks: (nodeId: string) => Promise<boolean | undefined>;
  selectLinkedNodes: (nodeId: string) => void;
  disconnectPin: (pinId: string) => Promise<boolean | undefined>;
  resetPinValue: (nodeId: string, pinId: string) => Promise<boolean | undefined>;
}
