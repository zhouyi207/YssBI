import { useTranslation } from "react-i18next";
import type { ResourceKind } from "@/shared/types/domain/resource";
import { useResourceRead } from "@/features/core/resource/read";
import { resourceKey } from "@/features/core/resource/resourceTypes";
import { Button } from "@/components/ui/button";
import { DetailPanelShell } from "../shared/DetailPanelShell";
import { DetailForm, DetailReadonlyField } from "../shared/DetailForm";

export function FileDetailPanel({
  resourceKind,
  resourceRef,
  status,
  onRetry,
}: {
  resourceKind: ResourceKind;
  resourceRef: string;
  status?: "loading" | "error" | "unavailable";
  onRetry?(): void;
}) {
  const { t } = useTranslation();
  const name = useResourceRead(
    (state) =>
      state.resources[resourceKey({ kind: resourceKind, id: resourceRef })]?.name ?? resourceRef,
  );
  return (
    <DetailPanelShell>
      <DetailForm>
        <DetailReadonlyField label={t("detail.fields.name")} tone="body">
          {name}
        </DetailReadonlyField>
      </DetailForm>
      {status && (
        <DetailForm>
          <p
            role={status === "loading" ? "status" : "alert"}
            className={
              status === "loading" ? "text-xs text-muted-foreground" : "text-xs text-destructive"
            }
          >
            {t(
              status === "loading"
                ? "common.loading"
                : status === "error"
                  ? "detail.loadFailed"
                  : "detail.unavailable",
            )}
          </p>
          {onRetry && status !== "loading" && (
            <Button size="sm" variant="outline" onClick={onRetry}>
              {t("common.retry")}
            </Button>
          )}
        </DetailForm>
      )}
    </DetailPanelShell>
  );
}
