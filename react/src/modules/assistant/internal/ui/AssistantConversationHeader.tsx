import { useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import {
  VscAdd,
  VscEllipsis,
  VscLock,
  VscUnlock,
  VscScreenFull,
  VscScreenNormal,
  VscCheck,
  VscClose,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import {
  DropdownMenu,
  DropdownMenuTrigger,
  DropdownMenuContent,
  DropdownMenuItem,
} from "@/components/ui/dropdown-menu";
import {
  createAssistantConversation,
  renameAssistantConversation,
  useAssistantConversationMutationState,
} from "@/features/application/assistant/assistantConversations";
import {
  openAssistantConversation,
  closeAssistantConversation,
} from "@/features/application/assistant/assistantConversationPanels";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import {
  workbenchLayoutRead,
  workbenchLayoutControl,
  showWorkbenchLayoutError,
  WORKBENCH_CONVERSATION_GROUP_ID,
} from "@/modules/workbench/public";
import { ui } from "@/features/core/ui/ui";

const readMaximized = () =>
  workbenchLayoutRead
    .listGroupPanels(WORKBENCH_CONVERSATION_GROUP_ID)
    .some((panel) => panel.maximized);

export function AssistantConversationHeader({
  sessionId,
  title,
}: {
  sessionId: string;
  title: string;
}) {
  const { t } = useTranslation();
  const [rename, setRename] = useState<string | null>(null);
  const mutation = useAssistantConversationMutationState();
  const options = useAssistantHarnessSnapshot((state) => state.turnOptions);
  const { setTurnOptions } = useAssistantHarnessActions();
  const maximized = useSyncExternalStore(
    workbenchLayoutRead.subscribe,
    readMaximized,
    readMaximized,
  );
  return (
    <>
      <header className="flex h-(--workbench-tab-height) min-h-8 shrink-0 items-center gap-1 border-b border-border px-3 text-xs">
        {rename === null ? (
          <span className="min-w-0 flex-1 truncate" title={title}>
            {title}
          </span>
        ) : (
          <form
            className="flex min-w-0 flex-1 items-center gap-1"
            onSubmit={async (event) => {
              event.preventDefault();
              const value = rename;
              if (await renameAssistantConversation(sessionId, value))
                setRename((current) => (current === value ? null : current));
            }}
          >
            <Input
              autoFocus
              className="h-6 min-w-0 text-xs"
              value={rename}
              onChange={(event) => setRename(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Escape") setRename(null);
              }}
              aria-label={t("panel.assistantRenameConversation")}
            />
            <Button
              type="submit"
              variant="ghost"
              size="icon-sm"
              disabled={mutation.saving}
              aria-label={t("common.save")}
            >
              <VscCheck aria-hidden />
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={() => setRename(null)}
              aria-label={t("common.cancel")}
            >
              <VscClose aria-hidden />
            </Button>
          </form>
        )}
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-pressed={options.mode === "ask"}
          title={t(`panel.assistantModeHint.${options.mode}`)}
          aria-label={t("panel.assistantReadOnly")}
          onClick={() =>
            setTurnOptions({ ...options, mode: options.mode === "ask" ? "write" : "ask" })
          }
        >
          {options.mode === "ask" ? (
            <VscLock aria-hidden className="text-primary" />
          ) : (
            <VscUnlock aria-hidden />
          )}
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          disabled={mutation.saving}
          title={t("panel.assistantNewConversation")}
          aria-label={t("panel.assistantNewConversation")}
          onClick={async () => {
            const session = await createAssistantConversation();
            if (session) await openAssistantConversation(session.sessionId, false);
          }}
        >
          <VscAdd aria-hidden />
        </Button>
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          aria-pressed={maximized}
          title={t(maximized ? "panel.assistantRestorePanel" : "panel.assistantExpandPanel")}
          aria-label={t(maximized ? "panel.assistantRestorePanel" : "panel.assistantExpandPanel")}
          onClick={() =>
            void workbenchLayoutControl
              .toggleConversationMaximized(sessionId)
              .catch(showWorkbenchLayoutError)
          }
        >
          {maximized ? <VscScreenNormal aria-hidden /> : <VscScreenFull aria-hidden />}
        </Button>
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              aria-label={t("panel.assistantConversationMenu")}
            >
              <VscEllipsis aria-hidden />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent align="end">
            <DropdownMenuItem onSelect={() => setRename(title)}>
              {t("panel.assistantRenameConversation")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => ui.showSettings()}>
              {t("panel.assistantOpenSettings")}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={() => void closeAssistantConversation(sessionId)}>
              {t("panel.assistantCloseConversation")}
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </header>
      {mutation.error && (
        <p role="alert" className="px-3 py-1 text-xs text-destructive">
          {t(`panel.assistantErrors.${mutation.error.code}`, {
            defaultValue: t("panel.assistantStatusError"),
          })}
        </p>
      )}
    </>
  );
}
