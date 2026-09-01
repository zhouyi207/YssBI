import i18n from "i18next";

import {
  WorkbenchLayoutError,
  type WorkbenchLayoutErrorCode,
} from "@/features/core/dockview/workbenchTypes";
import { uiStore } from "@/features/core/ui/UIStore";

const MESSAGE_KEYS = {
  dockview_not_ready: "workbench.layoutError.notReady",
  invalid_panel_metadata: "workbench.layoutError.invalidPanel",
  group_not_found: "workbench.layoutError.groupUnavailable",
  panel_open_failed: "workbench.layoutError.openFailed",
  layout_restore_failed: "workbench.layoutError.restoreFailed",
} as const satisfies Record<WorkbenchLayoutErrorCode, string>;

export function showWorkbenchLayoutError(error: unknown): void {
  const code = error instanceof WorkbenchLayoutError ? error.code : "panel_open_failed";
  void uiStore.alert({
    title: i18n.t("common.error"),
    message: i18n.t(MESSAGE_KEYS[code]),
    closeText: i18n.t("common.close"),
    type: "error",
  });
}
