import type { TFunction } from "i18next";
import type { ErrorReference } from "@/features/application/errorReference";

const CATALOG_ERROR_MESSAGE_KEYS: Readonly<Record<string, string>> = {
  compatible_draft_invalid: "nodeCatalog.invalidDraft",
  compatible_source_invalid: "nodeCatalog.invalidSource",
};

export function nodeCatalogErrorText(error: ErrorReference | null, t: TFunction): string {
  const genericText = error
    ? t(CATALOG_ERROR_MESSAGE_KEYS[error.code] ?? "nodeCatalog.loadError", {
        defaultValue: t("common.error"),
      })
    : t("nodeCatalog.loadError", { defaultValue: t("common.error") });
  if (!error) return genericText;

  const codeText = `[${error.code}]`;
  return error.incidentId
    ? `${genericText} ${codeText} · ${t("common.incidentId")}: ${error.incidentId}`
    : `${genericText} ${codeText}`;
}
