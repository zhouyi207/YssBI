import { useTranslation } from "react-i18next";
import { VscFile } from "react-icons/vsc";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

export function DetailEmptyState() {
  const { t } = useTranslation();

  return (
    <Empty className="rounded-none p-4 opacity-60">
      <EmptyHeader>
        <EmptyMedia variant="icon" className="size-10 text-muted-foreground">
          <VscFile className="size-5" />
        </EmptyMedia>
        <EmptyTitle>{t("detail.noSelection")}</EmptyTitle>
        <EmptyDescription>{t("detail.noSelectionHint")}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
