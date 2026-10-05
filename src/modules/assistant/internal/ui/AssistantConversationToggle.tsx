import { useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { VscCommentDiscussion } from "react-icons/vsc";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { toggleAssistantConversationWindow } from "@/features/application/assistant/assistantConversationPanels";
import {
  workbenchLayoutRead,
  showWorkbenchLayoutError,
  WORKBENCH_CONVERSATION_GROUP_ID,
} from "@/modules/workbench/public";
import { STATUS_BAR_ICON_SIZE } from "@/shared/theme/statusBarTokens";

const isOpen = () =>
  workbenchLayoutRead
    .listGroupPanels(WORKBENCH_CONVERSATION_GROUP_ID)
    .some((panel) => panel.visible);

export function AssistantConversationToggle() {
  const { t } = useTranslation();
  const open = useSyncExternalStore(workbenchLayoutRead.subscribe, isOpen, isOpen);
  const [pending, setPending] = useState(false);
  const title = t(open ? "bottomBar.closeConversation" : "bottomBar.openConversation");
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <button
          type="button"
          className="flexlayout__border_toolbar_button workbench-status-icon-button"
          data-workbench-conversation-toggle
          data-open={open}
          aria-label={title}
          aria-pressed={open}
          disabled={pending}
          onClick={async () => {
            if (pending) return;
            setPending(true);
            try {
              await toggleAssistantConversationWindow();
            } catch (error) {
              showWorkbenchLayoutError(error);
            } finally {
              setPending(false);
            }
          }}
        >
          <VscCommentDiscussion size={STATUS_BAR_ICON_SIZE} aria-hidden />
        </button>
      </TooltipTrigger>
      <TooltipContent side="top">{title}</TooltipContent>
    </Tooltip>
  );
}
