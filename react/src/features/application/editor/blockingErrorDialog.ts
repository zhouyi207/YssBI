import i18n from "i18next";
import { uiStore } from "@/features/core/ui/UIStore";
import { normalizeApplicationIpcError } from "@/features/application/errorReference";

export function showBlockingMessage(message: string): void {
  void uiStore.alert({
    title: i18n.t("common.error"),
    message,
    closeText: i18n.t("common.close"),
    type: "error",
  });
}

export function showBlockingIpcError(
  error: unknown,
  messageForCode: (code: string) => string,
): void {
  const reference = normalizeApplicationIpcError(error);
  void uiStore.alert({
    title: i18n.t("common.error"),
    message: messageForCode(reference.code),
    closeText: i18n.t("common.close"),
    type: "error",
    incidentId: reference.incidentId,
    incidentLabel: i18n.t("common.incidentId"),
  });
}
