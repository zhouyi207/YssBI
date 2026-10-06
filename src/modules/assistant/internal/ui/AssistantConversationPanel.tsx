import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { AssistantRuntimeProvider } from "@/features/application/assistant/AssistantRuntimeProvider";
import { useSidebarStore } from "@/features/core/sidebar/sidebarStore";
import { workbenchLayoutControl, showWorkbenchLayoutError } from "@/modules/workbench/public";
import { AssistantThread } from "./AssistantThread";
import { AssistantConversationHeader } from "./AssistantConversationHeader";

export function AssistantConversationPanel({ sessionId }: { sessionId: string }) {
  const { t } = useTranslation();
  const title = useSidebarStore((state) => {
    const row = state.panels.assistant?.snapshot?.document.rows.find(
      (row) =>
        row.kind === "item" && row.item.kind === "conversation" && row.item.sessionId === sessionId,
    );
    return row?.kind === "item" && row.item.kind === "conversation" ? row.item.title : undefined;
  });
  useEffect(() => {
    if (title !== undefined)
      void workbenchLayoutControl
        .updateConversationTitle({ sessionId, title: title || t("panel.assistantNewConversation") })
        .catch(showWorkbenchLayoutError);
  }, [sessionId, title, t]);
  return (
    <div
      className="assistant-conversation flex h-full min-h-0 w-full min-w-0 flex-col overflow-hidden bg-(--workbench-bg)"
      data-assistant-conversation={sessionId}
    >
      <AssistantRuntimeProvider key={sessionId} sessionId={sessionId}>
        <AssistantConversationHeader
          sessionId={sessionId}
          title={title || t("panel.assistantNewConversation")}
        />
        <AssistantThread />
      </AssistantRuntimeProvider>
    </div>
  );
}
