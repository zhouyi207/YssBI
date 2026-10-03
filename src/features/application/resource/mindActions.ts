import { MindService } from "@/services/mind/mindService";
import { createFileActions } from "./createFileActions";
import type { MindEdit, MindSnapshot } from "@/shared/types/domain/mind";
import { captureProjectCommandContext } from "@/features/application/projectCommandContext";
import { flushDocumentInputs } from "./documentInputs";
export const mindActions = createFileActions("mind", MindService);

export async function applyMindEdits(
  snapshot: MindSnapshot,
  edits: MindEdit[],
): Promise<MindSnapshot | null> {
  const context = captureProjectCommandContext();
  if (context.projectInstanceId !== snapshot.projectInstanceId) return null;
  await flushDocumentInputs(snapshot.path);
  context.assertCurrent();
  if (mindActions.getSnapshot(snapshot.path)?.version.sessionId !== snapshot.version.sessionId)
    return null;
  return mindActions.edit(snapshot.path, edits);
}

export async function deleteMindNodes(snapshot: MindSnapshot, nodeIds: readonly string[]) {
  const mind = snapshot.content;
  const byId = new Map(mind.nodes.map((node) => [node.id, node]));
  const selected = new Set(nodeIds.filter((id) => id !== mind.rootId && byId.has(id)));
  const edits: MindEdit[] = [];
  for (const id of selected) {
    let parentId = byId.get(id)?.parentId;
    while (parentId && !selected.has(parentId)) parentId = byId.get(parentId)?.parentId;
    if (!parentId) edits.push({ op: "remove_node", nodeId: id });
  }
  return edits.length ? applyMindEdits(snapshot, edits) : null;
}
