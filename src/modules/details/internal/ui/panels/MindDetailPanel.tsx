import { useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { Button } from "@/components/ui/button";
import { Select } from "@/shared/ui/Select";
import type { MindNode, MindSnapshot } from "@/shared/types/domain/mind";
import { useMindProjectionStore } from "@/features/core/resource/mindProjectionStore";
import { editorUi } from "@/features/core/editor/ui";
import { mindActions, applyMindEdits } from "@/features/application/resource/mindActions";
import { useFileTextInput } from "@/features/application/resource/useFileTextInput";
import { formatInlineUserError } from "@/features/application/userErrorSummary";
import { useEditorPaneStateStore } from "@/modules/workbench/public";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailForm, DetailTextarea } from "../shared/DetailForm";
import { DetailFieldRow } from "../shared/DetailFieldRow";
import { DetailEmptyState } from "../DetailEmptyState";

function MindNodeDetailForm({
  snapshot,
  node,
  panelInstanceId,
}: {
  snapshot: MindSnapshot;
  node: MindNode;
  panelInstanceId: string;
}) {
  const { t } = useTranslation();
  const textId = useId();
  const parentId = useId();
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const collapsed = useEditorPaneStateStore(
    (state) => state.collapsedNodeIds[panelInstanceId]?.includes(node.id) ?? false,
  );
  const input = useFileTextInput(
    snapshot,
    node.content,
    (content) => ({ op: "set_content" as const, nodeId: node.id, content }),
    mindActions,
    node.id,
  );
  const mind = snapshot.content;
  const descendants = new Set([node.id]);
  const childIds = new Map<string, string[]>();
  for (const item of mind.nodes)
    if (item.parentId)
      childIds.set(item.parentId, [...(childIds.get(item.parentId) ?? []), item.id]);
  const remaining = [node.id];
  while (remaining.length)
    for (const child of childIds.get(remaining.pop()!) ?? []) {
      if (!descendants.has(child)) {
        descendants.add(child);
        remaining.push(child);
      }
    }
  const siblings = mind.nodes.filter((item) => item.parentId === node.parentId);
  const siblingIndex = siblings.findIndex((item) => item.id === node.id);

  const run = (operation: () => Promise<unknown>) => {
    setError(null);
    setBusy(true);
    void operation()
      .catch((error) => setError(formatInlineUserError(error, t)))
      .finally(() => setBusy(false));
  };
  const selectAfterEdit = (nodeId: string) => {
    const focus = editorUi.getSnapshot().detailFocus;
    const ids = useEditorPaneStateStore.getState().selections[panelInstanceId]?.selectedNodeIds;
    const selected = ids ? (ids.length === 1 ? ids[0] : null) : mind.rootId;
    if (
      focus?.kind === "mind" &&
      focus.path === snapshot.path &&
      focus.panelInstanceId === panelInstanceId &&
      selected === node.id
    ) {
      useEditorPaneStateStore.getState().setSelectedNodeIds(panelInstanceId, [nodeId]);
      return true;
    }
    return false;
  };
  const add = (parentId: string) =>
    run(async () => {
      const id = crypto.randomUUID();
      const result = await applyMindEdits(snapshot, [
        { op: "add_node", node: { id, parentId, content: t("documents.newTopic") } },
      ]);
      if (!result) return;
      if (selectAfterEdit(id))
        useEditorPaneStateStore.getState().setNodeCollapsed(panelInstanceId, parentId, false);
    });
  const disabled = busy;

  return (
    <div
      className="divide-y divide-border/20"
      onKeyDown={(event) => {
        if (
          !event.defaultPrevented &&
          !event.shiftKey &&
          (event.ctrlKey || event.metaKey) &&
          event.key.toLowerCase() === "s"
        ) {
          event.preventDefault();
          event.stopPropagation();
          run(() => mindActions.save(snapshot.path));
        }
      }}
    >
      <DetailForm>
        <DetailFieldRow label={t("documents.nodeText")} htmlFor={textId}>
          <DetailTextarea
            id={textId}
            value={input.value}
            onChange={(event) => input.change(event.target.value)}
            onBlur={() =>
              void input.flush().catch((error) => setError(formatInlineUserError(error, t)))
            }
          />
        </DetailFieldRow>
        {node.parentId && (
          <DetailFieldRow label={t("documents.parent")} htmlFor={parentId}>
            <Select
              id={parentId}
              value={node.parentId}
              disabled={disabled}
              options={mind.nodes
                .filter((candidate) => !descendants.has(candidate.id))
                .map((candidate) => ({
                  value: candidate.id,
                  label: candidate.content || t("documents.newTopic"),
                }))}
              onChange={(parentId) =>
                run(() =>
                  applyMindEdits(snapshot, [
                    { op: "move_node", nodeId: node.id, parentId, beforeId: null },
                  ]),
                )
              }
            />
          </DetailFieldRow>
        )}
        {error && (
          <p role="alert" className="text-xs text-destructive">
            {error}
          </p>
        )}
      </DetailForm>
      <DetailForm>
        <div className="grid grid-cols-2 gap-2">
          <Button size="sm" variant="outline" disabled={disabled} onClick={() => add(node.id)}>
            {t("documents.addChild")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled || !node.parentId}
            onClick={() => node.parentId && add(node.parentId)}
          >
            {t("documents.addSibling")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled || !node.parentId || siblingIndex <= 0}
            onClick={() =>
              run(() =>
                applyMindEdits(snapshot, [
                  {
                    op: "move_node",
                    nodeId: node.id,
                    parentId: node.parentId!,
                    beforeId: siblings[siblingIndex - 1].id,
                  },
                ]),
              )
            }
          >
            {t("documents.moveUp")}
          </Button>
          <Button
            size="sm"
            variant="outline"
            disabled={disabled || !node.parentId || siblingIndex >= siblings.length - 1}
            onClick={() =>
              run(() =>
                applyMindEdits(snapshot, [
                  {
                    op: "move_node",
                    nodeId: node.id,
                    parentId: node.parentId!,
                    beforeId: siblings[siblingIndex + 2]?.id ?? null,
                  },
                ]),
              )
            }
          >
            {t("documents.moveDown")}
          </Button>
        </div>
        <Button
          size="sm"
          variant="outline"
          className="mt-1"
          onClick={() =>
            useEditorPaneStateStore
              .getState()
              .setNodeCollapsed(panelInstanceId, node.id, !collapsed)
          }
        >
          {t(collapsed ? "documents.expand" : "documents.collapse")}
        </Button>
        <Button
          size="sm"
          variant="outline"
          className="mt-1"
          disabled={disabled || node.position === undefined}
          onClick={() =>
            run(() =>
              applyMindEdits(snapshot, [{ op: "set_position", nodeId: node.id, position: null }]),
            )
          }
        >
          {t("documents.resetPosition")}
        </Button>
        <Button
          size="sm"
          variant="destructive"
          className="mt-1"
          disabled={disabled || node.id === mind.rootId}
          onClick={() =>
            run(async () => {
              const result = await applyMindEdits(snapshot, [
                { op: "remove_node", nodeId: node.id },
              ]);
              if (result) selectAfterEdit(node.parentId ?? mind.rootId);
            })
          }
        >
          {t("documents.deleteBranch")}
        </Button>
      </DetailForm>
    </div>
  );
}

export function MindDetailPanel({
  path,
  panelInstanceId,
}: {
  path: string;
  panelInstanceId: string;
}) {
  const { t } = useTranslation();
  const snapshot = useMindProjectionStore((state) => state.documents[path]);
  const selected = useEditorPaneStateStore(
    (state) => state.selections[panelInstanceId]?.selectedNodeIds,
  );
  const nodeId = selected ? (selected.length === 1 ? selected[0] : null) : snapshot?.content.rootId;
  const node = snapshot?.content.nodes.find((item) => item.id === nodeId);
  return (
    <DetailPanelShell>
      {snapshot && node ? (
        <MindNodeDetailForm
          key={`${snapshot.version.sessionId}:${node.id}`}
          snapshot={snapshot}
          node={node}
          panelInstanceId={panelInstanceId}
        />
      ) : snapshot ? (
        <DetailEmptyState />
      ) : (
        <p className="p-3 text-sm text-muted-foreground">{t("common.loading")}</p>
      )}
    </DetailPanelShell>
  );
}
