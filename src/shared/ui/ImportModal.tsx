import { useState } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose, VscFile, VscTable, VscCloudDownload, VscChevronRight } from "react-icons/vsc";
import { BsDatabaseFill } from "react-icons/bs";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { cn } from "@/lib/utils";
import { ImportDialogOptions, ImportDataSourceType } from "@/shared/types/ui";
import { ScrollArea } from "@/components/ui/scroll-area";

type CategoryId = "file" | "sql" | "other";

type ImportTypeConfig = {
  id: ImportDataSourceType;
  icon: React.ReactNode;
  tone: string;
  comingSoon: boolean;
};

const CATEGORIES: { id: CategoryId; icon: React.ReactNode; tone: string }[] = [
  { id: "file", icon: <VscFile size={16} />, tone: "text-emerald-600 bg-emerald-500/10 dark:text-emerald-300" },
  { id: "sql", icon: <BsDatabaseFill size={15} />, tone: "text-blue-600 bg-blue-500/10 dark:text-blue-300" },
  { id: "other", icon: <VscCloudDownload size={16} />, tone: "text-violet-600 bg-violet-500/10 dark:text-violet-300" },
];

const FILE_TYPES: ImportTypeConfig[] = [
  { id: "csv", icon: <VscFile size={20} />, tone: "text-emerald-600 bg-emerald-500/10 dark:text-emerald-300", comingSoon: false },
  { id: "xlsx", icon: <VscTable size={20} />, tone: "text-green-600 bg-green-500/10 dark:text-green-300", comingSoon: false },
];

const SQL_TYPES: ImportTypeConfig[] = [
  { id: "sqlite", icon: <BsDatabaseFill size={19} />, tone: "text-blue-600 bg-blue-500/10 dark:text-blue-300", comingSoon: false },
  { id: "postgres", icon: <BsDatabaseFill size={19} />, tone: "text-cyan-600 bg-cyan-500/10 dark:text-cyan-300", comingSoon: false },
  { id: "mysql", icon: <BsDatabaseFill size={19} />, tone: "text-orange-600 bg-orange-500/10 dark:text-orange-300", comingSoon: false },
  { id: "mariadb", icon: <BsDatabaseFill size={19} />, tone: "text-amber-600 bg-amber-500/10 dark:text-amber-300", comingSoon: false },
];

const OTHER_TYPES: ImportTypeConfig[] = [
  { id: "api", icon: <VscCloudDownload size={20} />, tone: "text-violet-600 bg-violet-500/10 dark:text-violet-300", comingSoon: true },
];

const CATEGORY_TYPES: Record<CategoryId, ImportTypeConfig[]> = {
  file: FILE_TYPES,
  sql: SQL_TYPES,
  other: OTHER_TYPES,
};

function TypeOptionCard({
  type,
  onSelect,
  onClose,
}: {
  type: ImportTypeConfig;
  onSelect: (id: ImportDataSourceType) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();
  const label = t(`importModal.types.${type.id}.label`);
  const description = t(`importModal.types.${type.id}.description`);

  return (
    <Button
      type="button"
      variant="outline"
      disabled={type.comingSoon}
      onClick={() => {
        if (type.comingSoon) return;
        onSelect(type.id);
        onClose();
      }}
      className={cn(
        "group h-auto min-h-[72px] w-full flex-col items-start gap-2 rounded-lg px-4 py-3 text-left",
        type.comingSoon && "cursor-default opacity-60",
      )}
    >
      <div className="flex w-full items-start gap-3">
        <div className={cn("flex h-9 w-9 shrink-0 items-center justify-center rounded-md", type.tone)}>
          {type.icon}
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <span className="text-sm font-medium text-foreground">{label}</span>
            {type.comingSoon && (
              <Badge variant="outline" className="h-5 px-1.5 text-[10px]">
                {t("importModal.developing")}
              </Badge>
            )}
          </div>
          <p className="mt-0.5 line-clamp-2 text-xs leading-relaxed text-muted-foreground">{description}</p>
        </div>
        {!type.comingSoon && (
          <VscChevronRight
            className="mt-1 shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-primary"
            size={16}
          />
        )}
      </div>
    </Button>
  );
}

export const ImportModal = ({ options, onClose }: { options: ImportDialogOptions; onClose: () => void }) => {
  const { t } = useTranslation();
  const [selectedCategory, setSelectedCategory] = useState<CategoryId>("file");
  const types = CATEGORY_TYPES[selectedCategory];

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="flex h-[min(480px,85vh)] max-w-[640px] flex-col gap-0 p-0">
        <DialogHeader className="shrink-0 border-b border-border px-6 py-4">
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0 space-y-1">
              <DialogTitle className="flex items-center gap-2 normal-case tracking-normal">
                <span className="flex h-8 w-8 items-center justify-center rounded-lg bg-primary/10 text-primary">
                  <VscDatabase size={18} />
                </span>
                {t("importModal.title")}
              </DialogTitle>
              <DialogDescription>{t("importModal.subtitle")}</DialogDescription>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              onClick={onClose}
              aria-label={t("importModal.close")}
              className="shrink-0"
            >
              <VscClose size={18} />
            </Button>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1">
          <nav className="flex w-[148px] shrink-0 flex-col gap-1 border-r border-border bg-muted/20 p-2">
            {CATEGORIES.map((cat) => {
              const active = selectedCategory === cat.id;
              return (
                <Button
                  key={cat.id}
                  type="button"
                  variant={active ? "secondary" : "ghost"}
                  onClick={() => setSelectedCategory(cat.id)}
                  className={cn(
                    "h-9 w-full justify-start gap-2 px-2.5 text-xs font-medium",
                    active && "bg-background shadow-sm",
                  )}
                >
                  <span className={cn("flex h-6 w-6 shrink-0 items-center justify-center rounded", cat.tone)}>
                    {cat.icon}
                  </span>
                  <span className="truncate">{t(`importModal.categories.${cat.id}`)}</span>
                </Button>
              );
            })}
          </nav>

          <ScrollArea className="min-h-0 flex-1">
            <div className="grid gap-2 p-4 sm:grid-cols-1">
              {types.map((type) => (
                <TypeOptionCard key={type.id} type={type} onSelect={options.onSelect} onClose={onClose} />
              ))}
            </div>
          </ScrollArea>
        </div>
      </DialogContent>
    </Dialog>
  );
};
