import type { ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { VscChevronRight } from "react-icons/vsc";
import { Button } from "@/components/ui/button";

export function SettingsPageHeader({
  breadcrumbs,
  actions,
}: {
  breadcrumbs: readonly { label: string; onSelect?: () => void }[];
  actions?: ReactNode;
}) {
  const { t } = useTranslation();

  return (
    <header className="settings-page-header">
      <nav aria-label={t("settings.breadcrumb")} className="min-w-0">
        <ol className="flex min-w-0 items-center gap-2 text-sm">
          {breadcrumbs.map((item, index) => (
            <li key={index} className="flex min-w-0 max-w-full items-center gap-2">
              {index > 0 && (
                <VscChevronRight aria-hidden className="shrink-0 text-muted-foreground" />
              )}
              {item.onSelect && index < breadcrumbs.length - 1 ? (
                <Button
                  type="button"
                  variant="link"
                  className="settings-breadcrumb-label shrink active:translate-y-0"
                  onClick={item.onSelect}
                  title={item.label}
                >
                  <span className="truncate">{item.label}</span>
                </Button>
              ) : (
                <span
                  aria-current={index === breadcrumbs.length - 1 ? "page" : undefined}
                  className="settings-breadcrumb-label"
                  title={item.label}
                >
                  <span className="truncate">{item.label}</span>
                </span>
              )}
            </li>
          ))}
        </ol>
      </nav>
      {actions && <div className="settings-page-actions">{actions}</div>}
    </header>
  );
}
