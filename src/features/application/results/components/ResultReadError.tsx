import { useTranslation } from "react-i18next";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import type { ErrorReference } from "@/shared/types/domain/diagnostics";

export function ResultReadError({ error }: { error: ErrorReference }) {
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
    </Alert>
  );
}
