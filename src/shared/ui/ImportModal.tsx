import { useState } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose, VscFile, VscTable, VscCloudDownload, VscChevronRight } from "react-icons/vsc";
import { BsDatabaseFill } from "react-icons/bs";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip";
import { cn } from "@/lib/utils";
import { ImportDialogOptions, ImportDataSourceType } from "@/shared/types/ui";
import { OverlayScrollbar } from "./OverlayScrollbar";

type ImportTypeConfig = {
  id: ImportDataSourceType;
  label: string;
  description: string;
  icon: React.ReactNode;
  tone: string;
  comingSoon: boolean;
};

type CategoryId = "file" | "sql" | "other";

const CATEGORIES: { id: CategoryId; label: string; icon: React.ReactNode; tone: string }[] = [
  { id: "file", label: "文件", icon: <VscFile size={17} />, tone: "text-emerald-600 bg-emerald-500/10 dark:text-emerald-300" },
  { id: "sql", label: "SQL 数据库", icon: <BsDatabaseFill size={16} />, tone: "text-blue-600 bg-blue-500/10 dark:text-blue-300" },
  { id: "other", label: "其他", icon: <VscCloudDownload size={17} />, tone: "text-violet-600 bg-violet-500/10 dark:text-violet-300" },
];

const FILE_TYPES: ImportTypeConfig[] = [
  { id: "csv", label: "CSV", description: "导入逗号分隔文本数据", icon: <VscFile size={22} />, tone: "text-emerald-600 bg-emerald-500/10 dark:text-emerald-300", comingSoon: false },
  { id: "xlsx", label: "Excel", description: "导入 Excel 工作簿或表格", icon: <VscTable size={22} />, tone: "text-green-600 bg-green-500/10 dark:text-green-300", comingSoon: false },
];

const SQL_TYPES: ImportTypeConfig[] = [
  { id: "sqlite", label: "SQLite", description: "连接本地 SQLite 数据库", icon: <BsDatabaseFill size={21} />, tone: "text-blue-600 bg-blue-500/10 dark:text-blue-300", comingSoon: false },
  { id: "postgres", label: "PostgreSQL", description: "连接 PostgreSQL 服务", icon: <BsDatabaseFill size={21} />, tone: "text-cyan-600 bg-cyan-500/10 dark:text-cyan-300", comingSoon: false },
  { id: "mysql", label: "MySQL", description: "连接 MySQL 数据库", icon: <BsDatabaseFill size={21} />, tone: "text-orange-600 bg-orange-500/10 dark:text-orange-300", comingSoon: false },
  { id: "mariadb", label: "MariaDB", description: "连接 MariaDB 数据库", icon: <BsDatabaseFill size={21} />, tone: "text-amber-600 bg-amber-500/10 dark:text-amber-300", comingSoon: false },
];

const OTHER_TYPES: ImportTypeConfig[] = [
  { id: "api", label: "REST API", description: "从远程接口拉取数据", icon: <VscCloudDownload size={22} />, tone: "text-violet-600 bg-violet-500/10 dark:text-violet-300", comingSoon: true },
];

const CATEGORY_TYPES: Record<CategoryId, ImportTypeConfig[]> = {
  file: FILE_TYPES,
  sql: SQL_TYPES,
  other: OTHER_TYPES,
};

function TypeOptionButton({
  type,
  onSelect,
  onClose,
}: {
  type: ImportTypeConfig;
  onSelect: (id: ImportDataSourceType) => void;
  onClose: () => void;
}) {
  const { t } = useTranslation();

  const button = (
    <Button
      type="button"
      variant="ghost"
      disabled={type.comingSoon}
      onClick={() => {
        onSelect(type.id);
        if (!type.comingSoon) onClose();
      }}
      className={cn(
        "group h-12 w-full justify-start gap-3 rounded-md px-2.5",
        type.comingSoon && "cursor-default opacity-50",
      )}
    >
      <div className={`flex h-8 w-8 shrink-0 items-center justify-center rounded-md ${type.tone}`}>{type.icon}</div>
      <div className="min-w-0 flex-1">
        <div className="flex items-center gap-2">
          <span className="text-[13px] font-medium text-foreground">{type.label}</span>
          {type.comingSoon && <Badge variant="outline" className="h-5 px-1.5 text-[10px]">{t("importModal.developing")}</Badge>}
        </div>
        <p className="truncate text-[11px] text-muted-foreground">{type.description}</p>
      </div>
      {!type.comingSoon && (
        <VscChevronRight className="shrink-0 text-muted-foreground transition-transform group-hover:translate-x-0.5 group-hover:text-[var(--accent-color)]" size={16} />
      )}
    </Button>
  );

  if (type.comingSoon) {
    return (
      <Tooltip>
        <TooltipTrigger asChild>{button}</TooltipTrigger>
        <TooltipContent side="right">{t("importModal.comingSoon")}</TooltipContent>
      </Tooltip>
    );
  }

  return button;
}

export const ImportModal = ({ options, onClose }: { options: ImportDialogOptions; onClose: () => void }) => {
  const { t } = useTranslation();
  const [selectedCategory, setSelectedCategory] = useState<CategoryId>("file");

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="h-[332px] max-w-[520px]">
        <DialogHeader className="shrink-0 border-b border-border bg-muted/20">
          <div className="flex items-center justify-between gap-4">
            <div>
              <DialogTitle className="flex items-center gap-2">
                <span className="flex h-7 w-7 items-center justify-center rounded-md bg-[var(--accent-color)]/12 text-[var(--accent-color)]">
                  <VscDatabase size={17} />
                </span>
                {t("importModal.title")}
              </DialogTitle>
            </div>
            <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label={t("importModal.close")}>
              <VscClose size={20} />
            </Button>
          </div>
        </DialogHeader>

        <Tabs
          value={selectedCategory}
          onValueChange={(value) => setSelectedCategory(value as CategoryId)}
          className="flex min-h-0 flex-1 flex-col gap-0"
        >
          <TabsList
            variant="line"
            className="h-auto w-full shrink-0 justify-start gap-1 rounded-none border-b border-border bg-transparent px-3 py-2"
          >
            {CATEGORIES.map((cat) => (
              <TabsTrigger key={cat.id} value={cat.id} className="h-8 gap-2 px-2.5 text-[12px]">
                <div className={`flex h-5 w-5 shrink-0 items-center justify-center rounded ${cat.tone}`}>
                  {cat.icon}
                </div>
                <span className="truncate font-medium">{t(`importModal.categories.${cat.id}`)}</span>
              </TabsTrigger>
            ))}
          </TabsList>

          {(Object.keys(CATEGORY_TYPES) as CategoryId[]).map((categoryId) => (
            <TabsContent key={categoryId} value={categoryId} className="mt-0 flex h-[232px] min-w-0 flex-col overflow-hidden">
              <OverlayScrollbar className="h-full">
                <div className="p-2">
                  <div className="space-y-1">
                    {CATEGORY_TYPES[categoryId].map((type) => (
                      <TypeOptionButton key={type.id} type={type} onSelect={options.onSelect} onClose={onClose} />
                    ))}
                  </div>
                </div>
              </OverlayScrollbar>
            </TabsContent>
          ))}
        </Tabs>
      </DialogContent>
    </Dialog>
  );
};
