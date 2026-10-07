import { createInstance } from "i18next";
import { expect, it } from "vitest";
import { enUS } from "@/app/i18n/locales/en-US";
import { zhCN } from "@/app/i18n/locales/zh-CN";
import { normalizeIpcError } from "@/services/ipc";
import { summarizeUserError } from "./userErrorSummary";

it("presents publication uncertainty without hiding the incident reference", async () => {
  const i18n = createInstance();
  await i18n.init({
    lng: "en-US",
    resources: { "en-US": { translation: enUS }, "zh-CN": { translation: zhCN } },
  });
  for (const language of ["en-US", "zh-CN"]) {
    await i18n.changeLanguage(language);
    for (const code of [
      "database_export_publication_uncertain",
      "plugin_file_publication_uncertain",
      "julia_worker_asset_publication_uncertain",
    ]) {
      const error = normalizeIpcError("export", {
        code,
        details: null,
        incidentId: "incident-publication",
      });
      const summary = summarizeUserError(error, i18n.t);
      expect(summary.message).toBe(i18n.t("common.filePublicationUncertain"));
      expect(summary.message).not.toBe("common.filePublicationUncertain");
      expect(summary.incidentId).toBe("incident-publication");
    }
  }
});
