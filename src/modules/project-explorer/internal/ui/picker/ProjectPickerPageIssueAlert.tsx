import { useTranslation } from "react-i18next";
import type { ProjectPickerPageIssue } from "@/features/application/project";
import { PageAlert } from "@/shared/ui/PageAlert";
import { ProjectPickerErrorDetails } from "./ProjectPickerFeedbackDetails";

const FAILURE_TITLE_KEYS: Record<
  Extract<ProjectPickerPageIssue, { kind: "failure" }>["operation"],
  string
> = {
  refresh: "notifications.projectPicker.refreshFailed",
  scan: "notifications.projectPicker.scanFailed",
  cleanup: "notifications.projectPicker.cleanupFailed",
  open: "notifications.projectPicker.openFailed",
  import: "notifications.projectPicker.importFailed",
  remove: "notifications.projectPicker.removeFailed",
  favorite: "notifications.projectPicker.favoriteFailed",
  reveal: "contextMenu.sidebar.revealInExplorerFailed",
};

function emptyIssueTitle(issue: Extract<ProjectPickerPageIssue, { kind: "empty" }>): {
  key: string;
  values?: Record<string, number>;
} {
  if (issue.operation === "cleanup") {
    return { key: "projectPicker.cleanupNone" };
  }
  if (issue.reason === "alreadyRegistered") {
    return {
      key: "projectPicker.scanAlreadyRegistered",
      values: { found: issue.found },
    };
  }
  return { key: "projectPicker.scanNoneFound" };
}

export function ProjectPickerPageIssueAlert({
  issue,
  onDismiss,
  onRetry,
}: {
  issue: ProjectPickerPageIssue;
  onDismiss: () => void;
  onRetry?: () => void;
}) {
  const { t } = useTranslation();

  if (issue.kind === "empty") {
    const title = emptyIssueTitle(issue);
    return (
      <PageAlert
        variant="info"
        title={t(title.key, title.values)}
        actionLabel={
          onRetry
            ? t("projectPicker.issues.retry", { defaultValue: t("common.refresh") })
            : undefined
        }
        onAction={onRetry}
        dismissLabel={t("common.close")}
        onDismiss={onDismiss}
      />
    );
  }

  const errorMessage = t(issue.error.messageKey, {
    defaultValue: t(issue.error.fallbackMessageKey),
  });
  return (
    <PageAlert
      variant="destructive"
      title={t(FAILURE_TITLE_KEYS[issue.operation], { error: errorMessage })}
      description={<ProjectPickerErrorDetails error={issue.error} />}
      actionLabel={
        onRetry ? t("projectPicker.issues.retry", { defaultValue: t("common.refresh") }) : undefined
      }
      onAction={onRetry}
      dismissLabel={t("common.close")}
      onDismiss={onDismiss}
    />
  );
}
