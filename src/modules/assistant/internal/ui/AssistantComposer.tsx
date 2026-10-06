import { AuiIf, ComposerPrimitive, useAui, useAuiState } from "@assistant-ui/react";
import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useShallow } from "zustand/react/shallow";
import {
  VscDebugStop,
  VscSend,
  VscClose,
  VscAdd,
  VscScreenFull,
  VscScreenNormal,
} from "react-icons/vsc";
import { Button } from "@/components/ui/button";
import {
  useAssistantHarnessActions,
  useAssistantHarnessSnapshot,
} from "@/features/application/assistant/AssistantRuntimeProvider";
import {
  AssistantResourcePicker,
  AssistantDraftReferences,
  AssistantReferenceChips,
} from "./AssistantReferences";
import { ui } from "@/features/core/ui/ui";
import { AssistantModelPicker } from "./AssistantModelPicker";
import { AssistantModePicker, AssistantEffortPicker } from "./AssistantRunOptions";
import { AssistantTokenUsage } from "./AssistantTokenUsage";

export function AssistantComposer() {
  const { t } = useTranslation();
  const aui = useAui();
  const [referencesOpen, setReferencesOpen] = useState(false);
  const [expanded, setExpanded] = useState(false);
  const snapshot = useAssistantHarnessSnapshot(
    useShallow((state) => ({
      status: state.status,
      error: state.error,
      isRunning: state.isRunning,
      isStopping: state.isStopping,
      selectingModel: state.selectingModel,
      activity: state.activity,
      compactionProgress: state.compactionProgress,
      recoveryAttempt: state.recoveryAttempt,
      unsentMessage: state.unsentMessage,
      queuedMessages: state.queuedMessages,
      interrupted: state.messages[state.messages.length - 1]?.status.type === "incomplete",
    })),
  );
  const actions = useAssistantHarnessActions();
  const canQueue = useAuiState((state) => state.composer.text.trim().length > 0);
  const sendDisabled = useAssistantHarnessSnapshot(
    (state) =>
      state.status !== "ready" ||
      !state.providerConfigured ||
      state.selectingModel ||
      state.isRunning ||
      state.isSubmitting,
  );
  const statusText = snapshot.error
    ? t(`panel.assistantErrors.${snapshot.error.code}`, {
        defaultValue: t("panel.assistantStatusError"),
      })
    : snapshot.status === "initializing"
      ? t("panel.assistantStatusInitializing")
      : snapshot.status === "provider-unavailable"
        ? t("panel.assistantStatusProviderUnavailable")
        : snapshot.isStopping
          ? t("panel.assistantStopping")
          : snapshot.isRunning && snapshot.activity
            ? t(`panel.assistantToolNames.${snapshot.activity}`, {
                defaultValue: t("panel.assistantStatusRunning"),
              }) +
              (snapshot.activity === "compacting" && snapshot.compactionProgress !== null
                ? ` ${snapshot.compactionProgress}%`
                : "") +
              (snapshot.activity === "reconnecting" && snapshot.recoveryAttempt > 0
                ? ` · ${t("panel.assistantRecoveryAttempt", { count: snapshot.recoveryAttempt })}`
                : "")
            : snapshot.isRunning
              ? t("panel.assistantStatusRunning")
              : snapshot.status === "ready"
                ? t("panel.assistantStatusReady")
                : t("panel.assistantStatusError");
  return (
    <div
      className="assistant-composer max-h-[75%] w-full shrink-0 overflow-y-auto border-t border-border bg-background px-3 py-2"
      data-expanded={expanded}
    >
      {snapshot.unsentMessage && !snapshot.isRunning && (
        <div className="mb-2 rounded-lg border border-border bg-muted/30 p-2 text-xs">
          <p className="font-medium">{t("panel.assistantUnsentMessage")}</p>
          <p className="mt-1 line-clamp-2 whitespace-pre-wrap wrap-anywhere text-muted-foreground">
            {snapshot.unsentMessage.text}
          </p>
          <AssistantReferenceChips resources={snapshot.unsentMessage.resources} />
          <Button
            type="button"
            variant="ghost"
            size="xs"
            onClick={() => {
              const draft = aui.composer.getState().text;
              const text = snapshot.unsentMessage?.text ?? "";
              aui.composer.setText(draft && draft !== text ? `${draft}\n\n${text}` : text);
              actions.restoreUnsentMessage();
            }}
          >
            {t("panel.assistantRestoreDraft")}
          </Button>
        </div>
      )}
      {snapshot.queuedMessages.length > 0 && (
        <div className="mb-2 rounded-lg border border-border p-2 text-xs">
          <p className="mb-1 font-medium">
            {t("panel.assistantQueuedMessages", { count: snapshot.queuedMessages.length })}
          </p>
          <div className="max-h-28 overflow-y-auto">
            {snapshot.queuedMessages.map((item) => (
              <div key={item.id} className="flex items-start gap-2 py-1">
                <div className="min-w-0 flex-1">
                  <p className="line-clamp-2 whitespace-pre-wrap wrap-anywhere">{item.text}</p>
                  <AssistantReferenceChips resources={item.resources} />
                </div>
                <Button
                  type="button"
                  variant="ghost"
                  size="icon-xs"
                  aria-label={t("panel.assistantRemoveQueued")}
                  onClick={() => actions.removeQueuedMessage(item.id)}
                >
                  <VscClose aria-hidden />
                </Button>
              </div>
            ))}
          </div>
          {!snapshot.isRunning && (
            <Button
              type="button"
              variant="ghost"
              size="xs"
              disabled={sendDisabled}
              onClick={() => void actions.sendNextQueued()}
            >
              {t("panel.assistantSendQueued")}
            </Button>
          )}
        </div>
      )}
      {(snapshot.error || snapshot.interrupted) && !snapshot.isRunning && (
        <div className="mb-2 flex flex-wrap gap-1">
          {snapshot.status === "error" && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              onClick={() => void actions.reconnect()}
            >
              {t("panel.assistantReloadConversations")}
            </Button>
          )}
          {snapshot.interrupted && (
            <Button
              type="button"
              variant="outline"
              size="xs"
              disabled={sendDisabled}
              onClick={() => void actions.continueTask(t("panel.assistantContinuePrompt"))}
            >
              {t("panel.assistantContinueTask")}
            </Button>
          )}
          <Button type="button" variant="ghost" size="xs" onClick={() => ui.showSettings()}>
            {t("panel.assistantOpenSettings")}
          </Button>
        </div>
      )}
      <ComposerPrimitive.Root className="relative mx-auto flex w-full max-w-3xl flex-col">
        <AssistantDraftReferences />
        <Button
          type="button"
          variant="ghost"
          size="icon-sm"
          className="absolute top-0 right-0 z-10 text-muted-foreground"
          aria-label={t(expanded ? "panel.assistantCollapseInput" : "panel.assistantExpandInput")}
          title={t(expanded ? "panel.assistantCollapseInput" : "panel.assistantExpandInput")}
          aria-expanded={expanded}
          onClick={() => setExpanded((value) => !value)}
        >
          {expanded ? <VscScreenNormal aria-hidden /> : <VscScreenFull aria-hidden />}
        </Button>
        <ComposerPrimitive.Input
          onKeyDown={(event) => {
            if (
              event.key === "@" &&
              !event.ctrlKey &&
              !event.altKey &&
              /(?:^|\s)$/.test(
                event.currentTarget.value.slice(0, event.currentTarget.selectionStart),
              )
            ) {
              event.preventDefault();
              setReferencesOpen(true);
            }
          }}
          aria-label={t("panel.assistantComposerLabel")}
          className={`w-full resize-none bg-transparent py-1 pr-8 text-[13px] leading-6 outline-none placeholder:text-muted-foreground ${expanded ? "min-h-[min(15rem,40vh)] max-h-[50vh]" : "min-h-24 max-h-44"}`}
          placeholder={t(
            snapshot.isRunning
              ? "panel.assistantComposerWhileRunning"
              : "panel.assistantComposerPlaceholder",
          )}
          submitMode="ctrlEnter"
        />
        {(snapshot.error || snapshot.status !== "ready" || snapshot.isRunning) && (
          <span
            role={snapshot.error ? "alert" : "status"}
            className={`min-w-0 py-1 text-[11px] leading-5 ${snapshot.error ? "text-destructive" : "text-muted-foreground"}`}
          >
            {statusText}
          </span>
        )}
        <div className="assistant-composer-toolbar flex min-w-0 flex-wrap items-center gap-1 pt-1">
          <div className="flex items-center gap-1">
            <AssistantResourcePicker open={referencesOpen} onOpenChange={setReferencesOpen} />
            <AssistantEffortPicker />
          </div>
          {snapshot.isRunning && (
            <Button
              type="button"
              variant="ghost"
              size="xs"
              disabled={!canQueue || snapshot.selectingModel}
              onClick={() => {
                actions.queueMessage(aui.composer.getState().text);
                aui.composer.setText("");
              }}
              title={t("panel.assistantQueueHint")}
            >
              <VscAdd aria-hidden />
              {t("panel.assistantQueueMessage")}
            </Button>
          )}
          <div className="ml-auto flex min-w-0 items-center gap-1">
            <AssistantTokenUsage />
            <AssistantModePicker />
            <AssistantModelPicker />
            <AuiIf condition={(state) => state.thread.isRunning}>
              <ComposerPrimitive.Cancel asChild>
                <Button
                  type="button"
                  size="icon-sm"
                  variant="ghost"
                  disabled={snapshot.isStopping}
                  aria-label={t("panel.assistantCancel")}
                  title={t("panel.assistantCancel")}
                >
                  <VscDebugStop aria-hidden />
                </Button>
              </ComposerPrimitive.Cancel>
            </AuiIf>
            <AuiIf condition={(state) => !state.thread.isRunning}>
              <ComposerPrimitive.Send asChild>
                <Button
                  type="submit"
                  size="icon-sm"
                  aria-label={t("panel.assistantSend")}
                  title={t("panel.assistantSend")}
                >
                  <VscSend aria-hidden />
                </Button>
              </ComposerPrimitive.Send>
            </AuiIf>
          </div>
        </div>
      </ComposerPrimitive.Root>
    </div>
  );
}
