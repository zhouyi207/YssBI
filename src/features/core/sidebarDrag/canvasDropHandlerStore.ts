/**
 * Store for registering the Canvas drop handler per group.
 * Workspace calls this when a node-template is dropped on the canvas.
 */
type DropHandler = (
  dragState: { type: string; template: any; x: number; y: number },
  event: { altKey: boolean; ctrlKey: boolean }
) => void | Promise<void>;

const handlers = new Map<string, DropHandler>();

export const canvasDropHandlerStore = {
  setHandler: (groupId: string, h: DropHandler | null) => {
    if (h) handlers.set(groupId, h);
    else handlers.delete(groupId);
  },
  getHandler: (groupId: string): DropHandler | null => handlers.get(groupId) ?? null,
};
