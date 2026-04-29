import { useState } from "react";
import { useTranslation } from "react-i18next";
import { VscDatabase, VscClose, VscFile, VscTable, VscCloudDownload } from "react-icons/vsc";
import { BsDatabaseFill } from "react-icons/bs";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent } from "@/components/ui/card";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { ImportDialogOptions, ImportDataSourceType } from "@/shared/types/ui";
import { OverlayScrollbar } from "./OverlayScrollbar";

type ImportTypeConfig = {
  id: ImportDataSourceType;
  label: string;
  icon: React.ReactNode;
  color: string;
  comingSoon: boolean;
};

type CategoryId = "file" | "sql" | "other";

const CATEGORIES: { id: CategoryId; label: string; icon: React.ReactNode; color: string }[] = [
  { id: "file", label: "文件", icon: <VscFile className="text-green-500" size={20} />, color: "bg-green-500/20" },
  { id: "sql", label: "SQL 数据库", icon: <BsDatabaseFill className="text-blue-500" size={20} />, color: "bg-blue-500/20" },
  { id: "other", label: "其他", icon: <VscCloudDownload className="text-purple-500" size={20} />, color: "bg-purple-500/20" },
];

const FILE_TYPES: ImportTypeConfig[] = [
  { id: "csv", label: "CSV", icon: <VscFile className="text-green-500" size={24} />, color: "bg-green-500/10", comingSoon: false },
  { id: "xlsx", label: "Excel", icon: <VscTable className="text-emerald-500" size={24} />, color: "bg-emerald-500/10", comingSoon: false },
];

const SQL_TYPES: ImportTypeConfig[] = [
  { id: "sqlite", label: "SQLite", icon: <BsDatabaseFill className="text-blue-500" size={24} />, color: "bg-blue-500/10", comingSoon: false },
  { id: "postgres", label: "PostgreSQL", icon: <BsDatabaseFill className="text-cyan-500" size={24} />, color: "bg-cyan-500/10", comingSoon: false },
  { id: "mysql", label: "MySQL", icon: <BsDatabaseFill className="text-orange-500" size={24} />, color: "bg-orange-500/10", comingSoon: false },
  { id: "mariadb", label: "MariaDB", icon: <BsDatabaseFill className="text-amber-500" size={24} />, color: "bg-amber-500/10", comingSoon: false },
];

const OTHER_TYPES: ImportTypeConfig[] = [
  { id: "api", label: "REST API", icon: <VscCloudDownload className="text-purple-500" size={24} />, color: "bg-purple-500/10", comingSoon: true },
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

  return (
    <Button
      type="button"
      variant="outline"
      onClick={() => {
        onSelect(type.id);
        if (!type.comingSoon) onClose();
      }}
      className={`group h-auto flex-col items-center gap-2 p-3 ${
        type.comingSoon
          ? "cursor-default opacity-55"
          : "hover:border-[var(--accent-color)] hover:bg-white/5"
      }`}
      title={type.comingSoon ? t("importModal.comingSoon") : undefined}
    >
      <div
        className={`flex items-center justify-center w-12 h-12 rounded-full ${type.color} transition-transform ${
          !type.comingSoon && "group-hover:scale-110"
        }`}
      >
        {type.icon}
      </div>
      <span className={`text-[11px] font-bold uppercase tracking-tight ${type.comingSoon ? "text-muted-foreground" : "text-gray-300 group-hover:text-white"}`}>
        {type.label}
        {type.comingSoon && <Badge variant="outline" className="mt-1 block w-fit">{t("importModal.developing")}</Badge>}
      </span>
    </Button>
  );
}

export const ImportModal = ({ options, onClose }: { options: ImportDialogOptions; onClose: () => void }) => {
  const { t } = useTranslation();
  const [selectedCategory, setSelectedCategory] = useState<CategoryId>("file");
  const types = CATEGORY_TYPES[selectedCategory];

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="h-[340px] max-w-[460px]">
        <DialogHeader className="shrink-0 border-b border-border bg-muted/20">
          <div className="flex items-center justify-between gap-4">
            <DialogTitle className="flex items-center gap-2">
              <VscDatabase className="text-blue-400" size={18} /> {t("importModal.title")}
            </DialogTitle>
            <Button type="button" variant="ghost" size="icon-sm" onClick={onClose} aria-label={t("importModal.close")}>
              <VscClose size={20} />
            </Button>
          </div>
        </DialogHeader>

        <div className="flex min-h-0 flex-1">
          <Card className="w-[132px] shrink-0 rounded-none border-0 border-r border-border bg-muted/20">
            <CardContent className="flex flex-col gap-1 p-2">
            {CATEGORIES.map((cat) => (
              <Button
                key={cat.id}
                type="button"
                variant={selectedCategory === cat.id ? "secondary" : "ghost"}
                onClick={() => setSelectedCategory(cat.id)}
                className="h-auto justify-start gap-2 px-2 py-2"
              >
                <div className={`w-8 h-8 rounded-lg flex items-center justify-center flex-shrink-0 ${cat.color}`}>
                  {cat.icon}
                </div>
                <span className="text-[11px] font-medium truncate">{t(`importModal.categories.${cat.id}`)}</span>
              </Button>
            ))}
            </CardContent>
          </Card>

          <div className="flex-1 flex flex-col min-w-0 overflow-hidden">
            <OverlayScrollbar className="h-full">
              <div className="p-4">
                <div className="grid grid-cols-2 gap-3">
                  {types.map((type) => (
                    <TypeOptionButton key={type.id} type={type} onSelect={options.onSelect} onClose={onClose} />
                  ))}
                </div>
              </div>
            </OverlayScrollbar>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
};
