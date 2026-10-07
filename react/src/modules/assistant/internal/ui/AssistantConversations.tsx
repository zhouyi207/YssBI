import { useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { VscCommentDiscussion, VscEdit } from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  createAssistantConversation,
  renameAssistantConversation,
  useAssistantConversationMutations,
} from "@/features/application/assistant/assistantConversations";
import { useActivityPanelDocument } from "@/features/application/sidebar/useActivityPanelDocument";
import type { ActivityItem } from "@/shared/types/domain/activityPanel";
import { openAssistantConversation } from "@/features/application/assistant/assistantConversationPanels";
import { assistantFailureKey } from "@/features/application/assistant/assistantMessageContent";
import {
  ActivityPanelDocumentView,
  workbenchLayoutRead,
  WORKBENCH_CONVERSATION_GROUP_ID,
} from "@/modules/workbench/public";

function selectedConversationId(): string | null {
  const panel = workbenchLayoutRead
    .listGroupPanels(WORKBENCH_CONVERSATION_GROUP_ID)
    .find((entry) => entry.visible);
  return panel?.metadata.role === "conversation" ? panel.metadata.sessionId : null;
}

export function AssistantConversations() {
  const { t, i18n } = useTranslation();
  const [query, setQuery] = useState("");
  const [rename, setRename] = useState<{ sessionId: string; title: string } | null>(null);
  const state = useAssistantConversationMutations();
  const panel = useActivityPanelDocument("assistant");
  const selectedId = useSyncExternalStore(
    workbenchLayoutRead.subscribe,
    selectedConversationId,
    selectedConversationId,
  );
  const matches = (item: ActivityItem) =>
    item.kind === "conversation" &&
    (item.title || t("panel.assistantNewConversation"))
      .toLocaleLowerCase()
      .includes(query.toLocaleLowerCase());
  const noMatches = Boolean(
    query &&
    panel.document?.rows.length &&
    !panel.document.rows.some((row) => row.kind === "item" && matches(row.item)),
  );
  return (
    <ActivityPanelDocumentView
      panelId="assistant"
      document={panel.document}
      error={panel.error}
      expanded={panel.expanded}
      onExpandedChange={panel.setExpanded}
      busy={panel.loading || state.saving}
      onRetry={panel.refresh}
      empty={panel.document?.rows.length === 0}
      filterItem={matches}
      actions={{
        newConversation: async () => {
          const session = await createAssistantConversation();
          if (session) await openAssistantConversation(session.sessionId, false);
        },
      }}
      notice={
        <>
          <div className="shrink-0 px-3 py-2">
            <Input
              value={query}
              onChange={(event) => setQuery(event.target.value)}
              placeholder={t("panel.assistantSearchConversations")}
              aria-label={t("panel.assistantSearchConversations")}
              className="h-7 text-xs"
            />
          </div>
          {state.error && (
            <div className="px-3 pb-2 text-xs text-destructive" role="alert">
              <p>{t(`panel.assistantErrors.${assistantFailureKey(state.error.code)}`)}</p>
            </div>
          )}
          {noMatches && (
            <p className="px-3 py-6 text-center text-xs text-muted-foreground">
              {t("panel.assistantNoMatchingConversations")}
            </p>
          )}
        </>
      }
      renderItem={(session) =>
        session.kind === "conversation" && (
          <div className="group mx-1 rounded hover:bg-muted/60">
            <div className="flex items-center gap-1 rounded has-aria-current:bg-muted">
              <button
                type="button"
                aria-current={session.sessionId === selectedId ? "true" : undefined}
                className="flex min-w-0 flex-1 items-start gap-2 rounded px-2 py-2 text-left outline-none focus-visible:ring-1 focus-visible:ring-ring"
                onClick={() => {
                  setRename(null);
                  void openAssistantConversation(session.sessionId, true);
                }}
              >
                <VscCommentDiscussion
                  aria-hidden
                  className="mt-0.5 shrink-0 text-muted-foreground"
                />
                <span className="min-w-0 flex-1">
                  <span className="block truncate text-xs">
                    {session.title || t("panel.assistantNewConversation")}
                  </span>
                  <span className="mt-1 block text-[11px] text-muted-foreground">
                    {new Intl.DateTimeFormat(i18n.language, {
                      dateStyle: "short",
                      timeStyle: "short",
                    }).format(session.lastOpenedAt)}
                  </span>
                </span>
              </button>
              <Button
                type="button"
                variant="ghost"
                size="icon-xs"
                className="mr-1 opacity-0 group-hover:opacity-100 focus-visible:opacity-100"
                disabled={state.saving}
                title={t("panel.assistantRenameConversation")}
                aria-label={t("panel.assistantRenameConversation")}
                onClick={() => setRename({ sessionId: session.sessionId, title: session.title })}
              >
                <VscEdit aria-hidden />
              </Button>
            </div>
            {rename?.sessionId === session.sessionId && (
              <form
                className="flex flex-wrap gap-1 px-2 pb-2"
                onSubmit={async (event) => {
                  event.preventDefault();
                  const editing = rename;
                  if (await renameAssistantConversation(editing.sessionId, editing.title))
                    setRename((current) => (current === editing ? null : current));
                }}
              >
                <Input
                  autoFocus
                  value={rename.title}
                  onChange={(event) => setRename({ ...rename, title: event.target.value })}
                  aria-label={t("panel.assistantRenameConversation")}
                  className="h-7 text-xs"
                />
                <Button type="submit" size="xs" disabled={state.saving || !rename.title.trim()}>
                  {t("panel.assistantSaveName")}
                </Button>
                <Button type="button" variant="ghost" size="xs" onClick={() => setRename(null)}>
                  {t("panel.assistantCancelEdit")}
                </Button>
              </form>
            )}
          </div>
        )
      }
    />
  );
}
