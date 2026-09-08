import type { TFunction } from "i18next";
import { describe, expect, it } from "vitest";
import { normalizeIpcError } from "@/services/ipc";
import { bayesActionErrorMessage } from "./bayesIssuePresentation";

const translate = ((key: string, options?: object) =>
  `${key} ${JSON.stringify(options ?? {})}`) as TFunction;

describe("Bayes issue privacy", () => {
  it("keeps incident identity without exposing backend details or transport prose", () => {
    const error = normalizeIpcError("export_bayes_artifact_csv", {
      code: "bayes_artifact_export_failed",
      details: { detail: "private backend detail" },
      incidentId: "incident-export-42",
    });

    const message = bayesActionErrorMessage(error, translate);
    expect(message).toContain("incident-export-42");
    expect(message).not.toContain("private backend detail");
    expect(
      bayesActionErrorMessage(new Error("private transport failure"), translate),
    ).not.toContain("private transport failure");
  });
});
