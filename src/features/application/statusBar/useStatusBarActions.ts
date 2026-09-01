import { useCallback } from "react";
import { useTranslation } from "react-i18next";
import { DEFAULT_VIEWPORT } from "@/shared/config-default";
import {
  captureActiveEditorCommandTarget,
  isEditorCommandTargetCurrent,
} from "@/features/application/editor/editorCommandFocus";
import { revealWorkbenchView } from "@/features/application/layout/workbenchLayoutActions";
import { setViewportLive, editorViewportScope } from "@/features/core/viewport";

/** Status bar command handlers — keeps StatusBar presentational. */
export function useStatusBarActions() {
  const { t } = useTranslation();

  const openLogsPanel = useCallback(() => {
    void revealWorkbenchView("logs");
  }, []);

  const resetCanvasViewport = useCallback(() => {
    const target = captureActiveEditorCommandTarget();
    if (!target || !isEditorCommandTargetCurrent(target)) return;
    if (target.resourceKind !== "event" && target.resourceKind !== "function") return;
    setViewportLive(editorViewportScope(target.groupId, target.resourceRef), {
      ...DEFAULT_VIEWPORT,
    });
  }, []);

  return {
    openLogsPanel,
    resetCanvasViewport,
    executionTooltip: t("bottomBar.openLogsPanel"),
    viewportTooltip: t("bottomBar.resetViewport"),
  };
}
