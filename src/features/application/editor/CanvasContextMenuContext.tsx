import { createContext, useContext } from "react";

export interface CanvasContextMenuActions {
  selectNode: (nodeId: string, groupId?: string) => void;
  copyNode: (nodeId: string) => void;
  cutNode: (nodeId: string) => Promise<void>;
  duplicateNode: (nodeId: string) => Promise<void>;
  deleteNode: (nodeId: string) => Promise<void>;
  breakAllNodeLinks: (nodeId: string) => Promise<void>;
  selectLinkedNodes: (nodeId: string) => void;
  disconnectPin: (pinId: string) => Promise<void>;
  resetPinValue: (nodeId: string, pinId: string) => Promise<void>;
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
