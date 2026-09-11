import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import type { ErrorReference } from "@/shared/types/domain/diagnostics";

export function ResultReadError({
  error,
  onRetry,
}: {
  error: ErrorReference;
  onRetry?: () => void;
}) {
  const { t } = useTranslation();

  return (
    <Alert variant="destructive">
      <AlertTitle>{t("resultSource.readFailed")}</AlertTitle>
      <AlertDescription>
        <p>
          {t("common.errorCode")}: <code>{error.code}</code>
        </p>
        {error.incidentId ? (
          <p>
            {t("common.incidentId")}: <code>{error.incidentId}</code>
          </p>
        ) : null}
      </AlertDescription>
      {onRetry && (
        <Button className="mt-2" size="sm" variant="outline" onClick={onRetry}>
          {t("common.retry")}
        </Button>
      )}
    </Alert>
  );
}
