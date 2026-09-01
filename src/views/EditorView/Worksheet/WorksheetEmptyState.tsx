import { useTranslation } from "react-i18next";
import { VscGraphLine } from "react-icons/vsc";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";

export function WorksheetEmptyState({ messageKey }: { messageKey?: string }) {
  const { t } = useTranslation();
  return (
    <Empty className="h-full min-h-0 rounded-none p-8">
      <EmptyHeader>
        <EmptyMedia variant="icon" className="size-12 text-muted-foreground">
          <VscGraphLine className="size-6" />
        </EmptyMedia>
        <EmptyTitle>{t(messageKey ?? "worksheet.previewEmpty")}</EmptyTitle>
        <EmptyDescription>{t("worksheet.previewEmptyHint")}</EmptyDescription>
      </EmptyHeader>
    </Empty>
  );
}
