import { createContext, useContext } from "react";

export interface CanvasContextMenuActions {
  selectNode: (nodeId: string, groupId?: string) => void;
  copyNode: (nodeId: string) => void;
  cutNode: (nodeId: string) => Promise<boolean | undefined>;
  duplicateNode: (nodeId: string) => Promise<boolean | undefined>;
  deleteNode: (nodeId: string) => Promise<boolean | undefined>;
  breakAllNodeLinks: (nodeId: string) => Promise<boolean | undefined>;
  selectLinkedNodes: (nodeId: string) => void;
  disconnectPin: (pinId: string) => Promise<boolean | undefined>;
  resetPinValue: (nodeId: string, pinId: string) => Promise<boolean | undefined>;
  removeRepeatablePin: (nodeId: string, pinId: string) => Promise<void>;
}

const CanvasContextMenuContext = createContext<CanvasContextMenuActions | null>(null);

export function CanvasContextMenuProvider({
  value,
  children,
}: {
  value: CanvasContextMenuActions;
  children: React.ReactNode;
}) {
  return (
    <CanvasContextMenuContext.Provider value={value}>
      {children}
    </CanvasContextMenuContext.Provider>
  );
}

export function useCanvasContextMenuActions(): CanvasContextMenuActions {
  const ctx = useContext(CanvasContextMenuContext);
  if (!ctx) {
    throw new Error("useCanvasContextMenuActions must be used within CanvasContextMenuProvider");
  }
  return ctx;
}

export function useCanvasContextMenuActionsOptional(): CanvasContextMenuActions | null {
  return useContext(CanvasContextMenuContext);
}
